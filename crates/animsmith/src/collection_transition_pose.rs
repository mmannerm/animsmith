//! Manifest-bound transition-pose collection execution.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

use animsmith_core::{
    CollectionDigestPinV1, CollectionTransitionPoseMemberInputV1, InputIdentity, LoadedSource,
    TransitionFamilyManifestIdentityV1, TransitionPoseDecisionV1, TransitionPoseStatusV1,
    evaluate_collection_transition_poses_v1,
};

use super::collection_manifest::{
    CollectionPathResolver, CollectionSourceResolution, load_collection_manifest_with_identity,
};
use super::collection_output::{
    COLLECTION_OUTPUT_MAX_AGGREGATE_SOURCE_BYTES, COLLECTION_OUTPUT_MAX_SOURCE_BYTES,
};
use super::transition_family::parse_collection_transition_families_bytes;
use super::{EXIT_FINDINGS, input_format, load_source_bytes_typed};

/// Run the strict collection transition-pose command.
pub(crate) fn run(manifest_path: &Path, families_path: &Path) -> Result<ExitCode, String> {
    let (exit, bytes) = run_with_source_loader(
        manifest_path,
        families_path,
        |_key, resolution, expected, limit| load_resolved_source(resolution, expected, limit),
    )?;
    super::publish::emit_required_json(&bytes)?;
    Ok(exit)
}

fn run_with_source_loader(
    manifest_path: &Path,
    families_path: &Path,
    mut load_selected_source: impl FnMut(
        &str,
        &CollectionSourceResolution,
        Option<&CollectionDigestPinV1>,
        u64,
    ) -> (SourceState, u64),
) -> Result<(ExitCode, Vec<u8>), String> {
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
    let clips = loaded_manifest
        .manifest
        .clips()
        .iter()
        .map(|clip| (clip.id().as_str(), clip))
        .collect::<BTreeMap<_, _>>();
    let manifest_sources = loaded_manifest
        .manifest
        .sources()
        .iter()
        .map(|source| (source.key().as_str(), source))
        .collect::<BTreeMap<_, _>>();

    // Complete the collection-specific control preflight before touching a
    // selected source. In particular, a stale later witness has control
    // precedence over an earlier member whose bytes cannot be read.
    let mut selected_source_keys = Vec::new();
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
            if !manifest_sources.contains_key(member.source().as_str()) {
                return Err(
                    "transition-family collection control error (stale-member-binding)".to_owned(),
                );
            }
            selected_source_keys.push(member.source().as_str());
        }
    }

    // Config paths are part of the established collection control plane even
    // though transition-pose V1 has no config-selected semantics. Validate
    // them only after every member witness is proven current, and before asset
    // bytes are read.
    for source in loaded_manifest.manifest.sources() {
        let config = resolver
            .resolve_config(source.config())
            .map_err(|error| error.to_string())?;
        super::collection_lint::load_collection_config_for_producer(config)?;
    }

    // Only a source that an already-validated declaration member selects is
    // runtime input. Unrelated manifest sources remain part of the path/config
    // control plane above but cannot consume bytes or influence this result.
    let mut primary_source_bytes = 0u64;
    let sources = load_unique_selected_sources(selected_source_keys, |key| {
        let source = manifest_sources
            .get(key)
            .ok_or_else(|| "collection control error (missing-source)".to_owned())?;
        let state = match next_source_limit(primary_source_bytes) {
            None => SourceState::Unavailable {
                input: None,
                cause: SourceUnavailableCause::SourceUnavailable,
            },
            Some(limit) => {
                let resolution = resolutions.get(key).ok_or_else(|| {
                    "collection control error (missing-source-resolution)".to_owned()
                })?;
                let (state, inspected) =
                    load_selected_source(key, resolution, source.expected_sha256(), limit);
                primary_source_bytes = primary_source_bytes
                    .checked_add(inspected)
                    .ok_or_else(|| "collection control error (source-work-overflow)".to_owned())?;
                state
            }
        };
        Ok::<SourceState, String>(state)
    })?;

    for family in declaration
        .declaration()
        .collection_families()
        .expect("checked collection")
    {
        for member in family.members() {
            let state = sources
                .get(member.source().as_str())
                .ok_or_else(|| "collection control error (missing-source-state)".to_owned())?;
            prepared.push(match state {
                SourceState::Available { loaded } => {
                    CollectionTransitionPoseMemberInputV1::available(
                        member.logical_id(),
                        member.source(),
                        member.take_index(),
                        member.take_name(),
                        loaded,
                    )
                }
                SourceState::DependencyClosureIncomplete { loaded } => {
                    CollectionTransitionPoseMemberInputV1::dependency_closure_incomplete(
                        member.logical_id(),
                        member.source(),
                        member.take_index(),
                        member.take_name(),
                        loaded,
                    )
                }
                SourceState::Unavailable {
                    input: Some(input),
                    cause,
                } => {
                    let _ = cause;
                    CollectionTransitionPoseMemberInputV1::unavailable_with_source_input(
                        member.logical_id(),
                        member.source(),
                        member.take_index(),
                        member.take_name(),
                        input,
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
    let manifest_authority = TransitionFamilyManifestIdentityV1::new(
        loaded_manifest.manifest.collection_id().clone(),
        loaded_manifest.input,
    )
    .map_err(|error| error.to_string())?;
    let result =
        evaluate_collection_transition_poses_v1(&declaration, &manifest_authority, &prepared)
            .map_err(|error| error.to_string())?;
    let pass = result.status() == TransitionPoseStatusV1::Complete
        && result.decision() == TransitionPoseDecisionV1::Pass;
    let bytes = super::publish::serialize_record(&result)?;
    let exit = if pass {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_FINDINGS)
    };
    Ok((exit, bytes))
}

enum SourceState {
    Available {
        loaded: LoadedSource,
    },
    DependencyClosureIncomplete {
        loaded: LoadedSource,
    },
    Unavailable {
        input: Option<InputIdentity>,
        cause: SourceUnavailableCause,
    },
}

#[derive(Clone, Copy)]
enum SourceUnavailableCause {
    SourceUnavailable,
}

fn load_source(
    path: &Path,
    expected: Option<&animsmith_core::CollectionDigestPinV1>,
    limit: u64,
) -> (SourceState, u64) {
    let bytes = match read_bounded(path, limit) {
        Ok(bytes) => bytes,
        Err(inspected) => {
            return (
                SourceState::Unavailable {
                    input: None,
                    cause: SourceUnavailableCause::SourceUnavailable,
                },
                inspected,
            );
        }
    };
    let inspected = bytes.len() as u64;
    let input = InputIdentity::from_bytes(&bytes);
    if expected.is_some_and(|expected| expected.as_str() != input.sha256()) {
        return (
            SourceState::Unavailable {
                input: Some(input),
                cause: SourceUnavailableCause::SourceUnavailable,
            },
            inspected,
        );
    }
    let Ok(format) = input_format(path) else {
        return (
            SourceState::Unavailable {
                input: Some(input),
                cause: SourceUnavailableCause::SourceUnavailable,
            },
            inspected,
        );
    };
    let Ok(loaded) = load_source_bytes_typed(path, format, &bytes) else {
        return (
            SourceState::Unavailable {
                input: Some(input),
                cause: SourceUnavailableCause::SourceUnavailable,
            },
            inspected,
        );
    };
    // Endpoint samples can depend on external resources, so preserve the
    // same loaded source for the core adapter rather than splitting document,
    // primary identity, and closure into independently pairable values.
    if loaded.dependency_closure().primary_input() != &input
        || !loaded.dependency_closure().coverage().is_complete()
        || loaded.dependency_closure().identity().is_none()
    {
        return (
            SourceState::DependencyClosureIncomplete { loaded },
            inspected,
        );
    }
    (SourceState::Available { loaded }, inspected)
}

fn load_resolved_source(
    resolution: &CollectionSourceResolution,
    expected: Option<&CollectionDigestPinV1>,
    limit: u64,
) -> (SourceState, u64) {
    match resolution {
        CollectionSourceResolution::Unavailable { .. } => (
            SourceState::Unavailable {
                input: None,
                cause: SourceUnavailableCause::SourceUnavailable,
            },
            0,
        ),
        CollectionSourceResolution::Ready(path) => load_source(path.path(), expected, limit),
    }
}

fn next_source_limit(primary_source_bytes: u64) -> Option<u64> {
    COLLECTION_OUTPUT_MAX_AGGREGATE_SOURCE_BYTES
        .checked_sub(primary_source_bytes)
        .map(|remaining| COLLECTION_OUTPUT_MAX_SOURCE_BYTES.min(remaining))
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, u64> {
    let file = File::open(path).map_err(|_| 0u64)?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| bytes.len() as u64)?;
    if bytes.len() as u64 > limit {
        return Err(bytes.len() as u64);
    }
    Ok(bytes)
}

fn load_unique_selected_sources<'a, T, E>(
    keys: impl IntoIterator<Item = &'a str>,
    mut load: impl FnMut(&'a str) -> Result<T, E>,
) -> Result<BTreeMap<String, T>, E> {
    let mut seen = BTreeSet::new();
    let mut loaded = BTreeMap::new();
    for key in keys {
        if seen.insert(key) {
            loaded.insert(key.to_owned(), load(key)?);
        }
    }
    Ok(loaded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_reader_counts_the_n_plus_one_terminal_witness() {
        let temp = tempfile::tempdir().expect("creates temporary source");
        let source = temp.path().join("source.glb");
        std::fs::write(&source, b"12").expect("writes synthetic source");

        assert_eq!(read_bounded(&source, 1), Err(2));
        let cap = COLLECTION_OUTPUT_MAX_AGGREGATE_SOURCE_BYTES;
        assert_eq!(next_source_limit(cap - 1), Some(1));
        assert_eq!(next_source_limit(cap + 1), None);
    }

    #[test]
    fn run_requests_only_unique_selected_source_keys() {
        let temp = tempfile::tempdir().expect("creates collection control root");
        let manifest = temp.path().join("collection.toml");
        let families = temp.path().join("families.toml");
        let manifest_bytes = br#"schema = "urn:animsmith:schema:collection-manifest:1"
schema_version = 1
collection_id = "test"
sources = [
  { key = "selected", path = "selected.glb" },
  { key = "unrelated", path = "unrelated.glb" },
]
clips = [
  { id = "test/walk", source = "selected", take_index = 0, take_name = "walk" },
  { id = "test/run", source = "selected", take_index = 1, take_name = "run" },
  { id = "test/unrelated", source = "unrelated", take_index = 0, take_name = "unused" },
]
"#;
        std::fs::write(&manifest, manifest_bytes).expect("writes collection manifest");
        let manifest_input = InputIdentity::from_bytes(manifest_bytes);
        std::fs::write(
            &families,
            format!(
                r#"schema = "urn:animsmith:schema:transition-family:1"
schema_version = 1
scope = "collection"
collection_id = "test"
manifest_input_identity = {{ sha256 = "{}", bytes = {} }}

[[families]]
family_id = "test/first"
boundary = "entry"
[families.basis]
translation = "skeleton-local-metres"
rotation = "skeleton-local-degrees"
time = "normalized-clip"
[families.tolerances]
translation_m = 0.0
rotation_deg = 0.0
time_normalized = 0.0
[[families.members]]
logical_id = "test/walk"
source = "selected"
take_index = 0
take_name = "walk"
[[families.members]]
logical_id = "test/run"
source = "selected"
take_index = 1
take_name = "run"

[[families]]
family_id = "test/second"
boundary = "exit"
[families.basis]
translation = "skeleton-local-metres"
rotation = "skeleton-local-degrees"
time = "normalized-clip"
[families.tolerances]
translation_m = 0.0
rotation_deg = 0.0
time_normalized = 0.0
[[families.members]]
logical_id = "test/run"
source = "selected"
take_index = 1
take_name = "run"
[[families.members]]
logical_id = "test/walk"
source = "selected"
take_index = 0
take_name = "walk"
"#,
                manifest_input.sha256(),
                manifest_input.bytes()
            ),
        )
        .expect("writes family declaration");
        let mut requested = Vec::new();

        let (_, bytes) = run_with_source_loader(
            &manifest,
            &families,
            |key, _resolution, _expected, _limit| {
                requested.push(key.to_owned());
                (
                    SourceState::Unavailable {
                        input: None,
                        cause: SourceUnavailableCause::SourceUnavailable,
                    },
                    0,
                )
            },
        )
        .expect("real run preflight accepts the declaration");

        assert_eq!(requested, ["selected"]);
        assert!(
            !bytes.is_empty(),
            "the run produced its shared result contract"
        );
    }
}
