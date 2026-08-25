//! Manifest-bound transition-pose collection execution.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

use animsmith_core::{
    CollectionTransitionPoseMemberInputV1, Document, InputIdentity, TransitionPoseDecisionV1,
    TransitionPoseStatusV1, evaluate_collection_transition_poses_v1,
};

use super::collection_manifest::{
    CollectionPathResolver, CollectionSourceResolution, load_collection_manifest_with_identity,
};
use super::collection_output::COLLECTION_OUTPUT_V2_MAX_SOURCE_BYTES;
use super::transition_family::parse_collection_transition_families_bytes;
use super::{EXIT_FINDINGS, input_format, load_source_bytes_typed};

/// Run the strict collection transition-pose command.
pub(crate) fn run(manifest_path: &Path, families_path: &Path) -> Result<ExitCode, String> {
    let loaded_manifest =
        load_collection_manifest_with_identity(manifest_path).map_err(|error| error.to_string())?;
    let family_bytes = read_bounded(
        families_path,
        animsmith_core::TRANSITION_FAMILY_V1_MAX_SOURCE_BYTES,
    )
    .map_err(|_| "transition-family collection control error (read)".to_owned())?;
    let declaration = parse_collection_transition_families_bytes(&family_bytes)
        .map_err(|error| error.to_string())?;
    let bound = match declaration.declaration() {
        animsmith_core::TransitionFamilyDeclarationV1::Collection { manifest, .. } => manifest,
        _ => return Err("transition-family collection control error (scope)".to_owned()),
    };
    if bound.collection_id() != loaded_manifest.manifest.collection_id()
        || bound.input() != &loaded_manifest.input
    {
        return Err(
            "transition-family collection control error (stale-manifest-binding)".to_owned(),
        );
    }
    let resolver =
        CollectionPathResolver::new(manifest_path, loaded_manifest.manifest.input_root())
            .map_err(|error| error.to_string())?;
    let resolutions = resolver
        .resolve_sources(loaded_manifest.manifest.sources())
        .map_err(|error| error.to_string())?;
    // Config paths are part of the established collection control plane even
    // though transition-pose V1 has no config-selected semantics.
    for source in loaded_manifest.manifest.sources() {
        let config = resolver
            .resolve_config(source.config())
            .map_err(|error| error.to_string())?;
        super::collection_lint::load_collection_config_for_producer(config)?;
    }
    let mut sources = BTreeMap::new();
    for source in loaded_manifest.manifest.sources() {
        let resolution = resolutions
            .get(source.key().as_str())
            .ok_or_else(|| "collection control error (missing-source-resolution)".to_owned())?;
        let state = match resolution {
            CollectionSourceResolution::Unavailable { .. } => SourceState::Unavailable {
                input: None,
                cause: SourceUnavailableCause::SourceUnavailable,
            },
            CollectionSourceResolution::Ready(path) => {
                load_source(path.path(), source.expected_sha256())
            }
        };
        sources.insert(source.key().as_str().to_owned(), state);
    }
    let clips = loaded_manifest
        .manifest
        .clips()
        .iter()
        .map(|clip| (clip.id().as_str(), clip))
        .collect::<BTreeMap<_, _>>();
    let mut prepared = Vec::new();
    for family in declaration
        .declaration()
        .collection_families()
        .expect("checked collection")
    {
        for member in family.members() {
            let manifest_clip = clips.get(member.logical_id().as_str()).ok_or_else(|| {
                "transition-family collection control error (stale-member-binding)".to_owned()
            })?;
            if manifest_clip.source() != member.source()
                || u64::from(manifest_clip.take_index()) != member.take_index()
                || manifest_clip.take_name() != member.take_name()
            {
                return Err(
                    "transition-family collection control error (stale-member-binding)".to_owned(),
                );
            }
            let state = sources.get(member.source().as_str()).ok_or_else(|| {
                "transition-family collection control error (missing-source-state)".to_owned()
            })?;
            prepared.push(match state {
                SourceState::Available { input, document } => {
                    CollectionTransitionPoseMemberInputV1::available(
                        member.logical_id(),
                        member.source(),
                        member.take_index(),
                        member.take_name(),
                        &input,
                        &document,
                    )
                }
                SourceState::Unavailable {
                    input: Some(input),
                    cause,
                } => {
                    // #573's collection member API will map
                    // DependencyClosureIncomplete to its typed family reason.
                    // Until then this state deliberately remains a normal
                    // unavailable member rather than a false complete result.
                    let _ = cause;
                    CollectionTransitionPoseMemberInputV1::unavailable_with_source_input(
                        member.logical_id(),
                        member.source(),
                        member.take_index(),
                        member.take_name(),
                        &input,
                    )
                }
                SourceState::Unavailable { input: None, .. } => {
                    CollectionTransitionPoseMemberInputV1::unavailable(
                        member.logical_id(),
                        member.source(),
                        member.take_index(),
                        member.take_name(),
                    )
                }
            });
        }
    }
    let result =
        evaluate_collection_transition_poses_v1(&declaration, loaded_manifest.input, &prepared)
            .map_err(|error| error.to_string())?;
    let pass = result.status() == TransitionPoseStatusV1::Complete
        && result.decision() == TransitionPoseDecisionV1::Pass;
    let bytes = super::publish::serialize_record(&result)?;
    super::publish::emit_required_json(&bytes)?;
    Ok(if pass {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_FINDINGS)
    })
}

enum SourceState {
    Available {
        input: InputIdentity,
        document: Document,
    },
    Unavailable {
        input: Option<InputIdentity>,
        cause: SourceUnavailableCause,
    },
}

#[derive(Clone, Copy)]
enum SourceUnavailableCause {
    SourceUnavailable,
    DependencyClosureIncomplete,
}

fn load_source(
    path: &Path,
    expected: Option<&animsmith_core::CollectionDigestPinV1>,
) -> SourceState {
    let Ok(bytes) = read_bounded(path, COLLECTION_OUTPUT_V2_MAX_SOURCE_BYTES) else {
        return SourceState::Unavailable {
            input: None,
            cause: SourceUnavailableCause::SourceUnavailable,
        };
    };
    let input = InputIdentity::from_bytes(&bytes);
    if expected.is_some_and(|expected| expected.as_str() != input.sha256()) {
        return SourceState::Unavailable {
            input: Some(input),
            cause: SourceUnavailableCause::SourceUnavailable,
        };
    }
    let Ok(format) = input_format(path) else {
        return SourceState::Unavailable {
            input: Some(input),
            cause: SourceUnavailableCause::SourceUnavailable,
        };
    };
    let Ok(loaded) = load_source_bytes_typed(path, format, &bytes) else {
        return SourceState::Unavailable {
            input: Some(input),
            cause: SourceUnavailableCause::DependencyClosureIncomplete,
        };
    };
    // Endpoint samples can depend on external resources. The primary identity
    // alone is therefore insufficient: until the core V1 result carries the
    // complete closure identity, reject a partial/unidentified closure rather
    // than blessing a primary-only comparison. The follow-up core amendment
    // will retain this already validated identity in each available member.
    if loaded.dependency_closure().primary_input() != &input
        || !loaded.dependency_closure().coverage().is_complete()
        || loaded.dependency_closure().identity().is_none()
    {
        return SourceState::Unavailable {
            input: Some(input),
            cause: SourceUnavailableCause::SourceUnavailable,
        };
    }
    SourceState::Available {
        input,
        document: loaded.into_document(),
    }
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, ()> {
    let file = File::open(path).map_err(|_| ())?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    if bytes.len() as u64 > limit {
        return Err(());
    }
    Ok(bytes)
}
