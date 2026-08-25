//! Collection-manifest execution and aggregate failure routing.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use animsmith_core::{
    CheckSelection, CollectionClipV1, CollectionDigestPinV1, CollectionManifestV1,
    CollectionSourceV1, InputIdentity, LintEnvelope, Severity, SourceObservationStateV1,
    SourceSetCoverageStateV1,
};

use super::collection_manifest::{
    CollectionConfigResolution, CollectionPathResolver, CollectionSourceResolution,
    CollectionSourceUnavailable, load_collection_manifest_with_identity,
};
use super::collection_output::{
    COLLECTION_OUTPUT_V2_MAX_AGGREGATE_SOURCE_BYTES, COLLECTION_OUTPUT_V2_MAX_SOURCE_BYTES,
    CheckReferenceState, CheckReferenceUnavailableReason, ClipBindingState, ClipUnavailableReason,
    CollectionClipRecord, CollectionManifestIdentity, CollectionOutput, CollectionRuntimeSetRecord,
    CollectionSourceRecord, ConfigState, DigestPinState, DocumentResult, DocumentUnavailableReason,
    LoaderState, LoaderUnavailableReason, MeasurementReference, NormalizedClipState, ObservedTake,
    RuntimeSetMember, RuntimeSetMemberState, SourceInputState, SourceUnavailableReason,
    TakeInventoryState, TakeNameState,
};
use super::{
    EXIT_FINDINGS, InputFormat, LintAnalysis, LoadedConfig, LoadedInput, analyze_loaded_lint,
    current_tool, full_check_ids, input_format, load_source_bytes_typed, parse_config,
};

const COLLECTION_CONFIG_MAX_BYTES: u64 = animsmith_core::COLLECTION_MANIFEST_V1_MAX_MANIFEST_BYTES;

struct PreparedConfig {
    loaded: LoadedConfig,
    evidence: ConfigState,
}

struct ReadySource {
    loaded: LoadedInput,
    indexed_measurements: Vec<animsmith_core::measure::ClipMeasurements>,
    digest_mismatched: bool,
    nested_output_available: bool,
    duplicate_normalized_names: BTreeSet<String>,
}

enum ExecutedSource {
    Ready(Box<ReadySource>),
    Unavailable(ClipUnavailableReason),
}

pub(crate) fn run_collection_lint(manifest_path: &Path) -> Result<ExitCode, String> {
    full_check_ids()?;
    let loaded_manifest =
        load_collection_manifest_with_identity(manifest_path).map_err(|error| error.to_string())?;
    let manifest = loaded_manifest.manifest;
    let manifest_identity =
        CollectionManifestIdentity::new(manifest.collection_id().as_str(), loaded_manifest.input);

    // Finish the entire control-plane phase before reading any primary source.
    let resolver = CollectionPathResolver::new(manifest_path, manifest.input_root())
        .map_err(|error| error.to_string())?;
    let source_resolutions = resolver
        .resolve_sources(manifest.sources())
        .map_err(|error| error.to_string())?;
    let mut prepared_configs = BTreeMap::new();
    for source in manifest.sources() {
        let resolution = resolver
            .resolve_config(source.config())
            .map_err(|error| error.to_string())?;
        prepared_configs.insert(
            source.key().as_str().to_owned(),
            prepare_config(resolution)?,
        );
    }

    let mut source_records = Vec::with_capacity(manifest.sources().len());
    let mut executions = BTreeMap::new();
    let mut primary_source_bytes = 0u64;
    let mut requires_failure = false;
    for source in manifest.sources() {
        let key = source.key().as_str();
        let config = prepared_configs
            .remove(key)
            .ok_or_else(|| "collection control error (missing-config-basis)".to_owned())?;
        let resolution = source_resolutions
            .get(key)
            .ok_or_else(|| "collection control error (missing-source-resolution)".to_owned())?;
        let Some(source_limit) = next_source_limit(primary_source_bytes) else {
            let execution = unavailable_source_record(
                source,
                config.evidence,
                SourceInputState::Unavailable {
                    reason: SourceUnavailableReason::AggregateExhausted,
                    inspected_bytes: 0,
                },
                digest_state(source.expected_sha256(), None),
                LoaderUnavailableReason::SourceUnavailable,
                ClipUnavailableReason::SourceUnavailable,
            );
            requires_failure = true;
            source_records.push(execution.record);
            executions.insert(key.to_owned(), execution.runtime);
            continue;
        };
        let execution = execute_source(
            source,
            resolution,
            config,
            source_limit,
            &mut primary_source_bytes,
        )?;
        requires_failure |= execution.requires_failure;
        source_records.push(execution.record);
        executions.insert(key.to_owned(), execution.runtime);
    }

    let (clip_records, clip_states) = bind_clips(&manifest, &executions);
    requires_failure |= clip_states
        .values()
        .any(|state| !matches!(state, RuntimeSetMemberState::Established));
    let runtime_sets = manifest
        .runtime_sets()
        .iter()
        .map(|runtime_set| {
            let members = runtime_set
                .members()
                .iter()
                .map(|member| {
                    RuntimeSetMember::new(
                        member.as_str(),
                        clip_states.get(member.as_str()).cloned().unwrap_or(
                            RuntimeSetMemberState::Unavailable {
                                reason: ClipUnavailableReason::DocumentUnavailable,
                            },
                        ),
                    )
                })
                .collect();
            CollectionRuntimeSetRecord::new(runtime_set.id().as_str(), runtime_set.kind(), members)
        })
        .collect::<Vec<_>>();

    let mut output = CollectionOutput::new(
        current_tool(),
        manifest_identity,
        source_records,
        clip_records,
        runtime_sets,
        primary_source_bytes,
        0,
    )
    .map_err(|error| error.to_string())?;
    let bytes = output
        .render_json_vec()
        .map_err(|error| error.to_string())?;
    super::collection_output::read_collection_output(Cursor::new(&bytes))
        .map_err(|error| error.to_string())?;
    super::publish::emit(&bytes);
    Ok(if requires_failure {
        ExitCode::from(EXIT_FINDINGS)
    } else {
        ExitCode::SUCCESS
    })
}

fn next_source_limit(primary_source_bytes: u64) -> Option<u64> {
    let remaining =
        COLLECTION_OUTPUT_V2_MAX_AGGREGATE_SOURCE_BYTES.checked_sub(primary_source_bytes)?;
    Some(COLLECTION_OUTPUT_V2_MAX_SOURCE_BYTES.min(remaining))
}

struct SourceExecution {
    record: CollectionSourceRecord,
    runtime: ExecutedSource,
    requires_failure: bool,
}

fn execute_source(
    source: &CollectionSourceV1,
    resolution: &CollectionSourceResolution,
    config: PreparedConfig,
    source_limit: u64,
    primary_source_bytes: &mut u64,
) -> Result<SourceExecution, String> {
    let (resolved, unavailable_reason) = match resolution {
        CollectionSourceResolution::Ready(path) => (Some(path), None),
        CollectionSourceResolution::Unavailable { reason, .. } => {
            let reason = match reason {
                CollectionSourceUnavailable::Missing => SourceUnavailableReason::Missing,
                CollectionSourceUnavailable::Unreadable => SourceUnavailableReason::Unreadable,
            };
            (None, Some(reason))
        }
    };
    let Some(resolved) = resolved else {
        return Ok(unavailable_source_record(
            source,
            config.evidence,
            SourceInputState::Unavailable {
                reason: unavailable_reason.expect("unavailable resolution has a reason"),
                inspected_bytes: 0,
            },
            digest_state(source.expected_sha256(), None),
            LoaderUnavailableReason::SourceUnavailable,
            ClipUnavailableReason::SourceUnavailable,
        ));
    };

    let bytes = match read_primary_bounded(resolved.path(), source_limit) {
        Ok(bytes) => {
            *primary_source_bytes = primary_source_bytes
                .checked_add(bytes.len() as u64)
                .ok_or_else(|| "collection control error (source-work-overflow)".to_owned())?;
            bytes
        }
        Err((reason, inspected_bytes)) => {
            *primary_source_bytes = primary_source_bytes
                .checked_add(inspected_bytes)
                .ok_or_else(|| "collection control error (source-work-overflow)".to_owned())?;
            return Ok(unavailable_source_record(
                source,
                config.evidence,
                SourceInputState::Unavailable {
                    reason,
                    inspected_bytes,
                },
                digest_state(source.expected_sha256(), None),
                LoaderUnavailableReason::SourceUnavailable,
                ClipUnavailableReason::SourceUnavailable,
            ));
        }
    };
    let input = InputIdentity::from_bytes(&bytes);
    let digest = digest_state(source.expected_sha256(), Some(&input));
    let digest_mismatched = matches!(digest, DigestPinState::Mismatched { .. });
    let format = match input_format(resolved.path()) {
        Ok(format) => format,
        Err(_) => {
            return Ok(loader_unavailable_source_record(
                source,
                config.evidence,
                input,
                digest,
                LoaderUnavailableReason::UnsupportedFormat,
            ));
        }
    };
    let loaded_source = match load_source_bytes_typed(resolved.path(), format, &bytes) {
        Ok(source) => source,
        Err(error) => {
            return Ok(loader_unavailable_source_record(
                source,
                config.evidence,
                input,
                digest,
                classify_loader_error(format, &error),
            ));
        }
    };
    let engine = config.loaded.resolve_engine_input(
        loaded_source.source_facts().format(),
        loaded_source.document(),
    )?;
    let loaded = LoadedInput {
        source: loaded_source,
        engine,
    };
    let analysis = analyze_loaded_lint(
        &loaded,
        &config.loaded,
        source.path().as_str(),
        CheckSelection::All,
        Severity::Error,
        &BTreeSet::new(),
    )?;
    let LintAnalysis {
        report,
        requires_failure,
        indexed_measurements,
    } = analysis;
    let envelope =
        LintEnvelope::new(current_tool(), vec![report]).map_err(|error| error.to_string())?;
    let nested_output_available =
        loaded.document().clips.iter().all(|clip| {
            clip.name.len() <= animsmith_core::COLLECTION_MANIFEST_V1_MAX_TAKE_NAME_BYTES
        });
    let (take_inventory, observed_takes) = observed_takes(&loaded);
    let duplicate_normalized_names = duplicate_clip_names(&loaded);
    let record = CollectionSourceRecord::new(
        source.key().as_str(),
        source.path().as_str(),
        SourceInputState::Available {
            input: input.clone(),
        },
        digest,
        config.evidence,
        LoaderState::Ready,
        take_inventory,
        observed_takes,
        if nested_output_available {
            DocumentResult::Available {
                envelope: Box::new(envelope),
            }
        } else {
            DocumentResult::Unavailable {
                reason: DocumentUnavailableReason::NestedOutput,
            }
        },
    );
    Ok(SourceExecution {
        record,
        runtime: ExecutedSource::Ready(Box::new(ReadySource {
            loaded,
            indexed_measurements,
            digest_mismatched,
            nested_output_available,
            duplicate_normalized_names,
        })),
        requires_failure: requires_failure || digest_mismatched || !nested_output_available,
    })
}

fn unavailable_source_record(
    source: &CollectionSourceV1,
    config: ConfigState,
    input: SourceInputState,
    digest: DigestPinState,
    loader_reason: LoaderUnavailableReason,
    clip_reason: ClipUnavailableReason,
) -> SourceExecution {
    SourceExecution {
        record: CollectionSourceRecord::new(
            source.key().as_str(),
            source.path().as_str(),
            input,
            digest,
            config,
            LoaderState::Unavailable {
                reason: loader_reason,
            },
            TakeInventoryState::Unavailable,
            Vec::new(),
            DocumentResult::Unavailable {
                reason: DocumentUnavailableReason::Source,
            },
        ),
        runtime: ExecutedSource::Unavailable(clip_reason),
        requires_failure: true,
    }
}

fn loader_unavailable_source_record(
    source: &CollectionSourceV1,
    config: ConfigState,
    input: InputIdentity,
    digest: DigestPinState,
    reason: LoaderUnavailableReason,
) -> SourceExecution {
    SourceExecution {
        record: CollectionSourceRecord::new(
            source.key().as_str(),
            source.path().as_str(),
            SourceInputState::Available { input },
            digest,
            config,
            LoaderState::Unavailable { reason },
            TakeInventoryState::Unavailable,
            Vec::new(),
            DocumentResult::Unavailable {
                reason: DocumentUnavailableReason::Loader,
            },
        ),
        runtime: ExecutedSource::Unavailable(ClipUnavailableReason::LoaderUnavailable),
        requires_failure: true,
    }
}

fn bind_clips(
    manifest: &CollectionManifestV1,
    executions: &BTreeMap<String, ExecutedSource>,
) -> (
    Vec<CollectionClipRecord>,
    BTreeMap<String, RuntimeSetMemberState>,
) {
    let mut states = BTreeMap::new();
    let records = manifest
        .clips()
        .iter()
        .map(|clip| {
            let binding = executions
                .get(clip.source().as_str())
                .map(|execution| bind_clip(clip, execution))
                .unwrap_or(ClipBindingState::Unavailable {
                    reason: ClipUnavailableReason::DocumentUnavailable,
                });
            let member_state = match &binding {
                ClipBindingState::Established { .. } => RuntimeSetMemberState::Established,
                ClipBindingState::Unavailable { reason } => {
                    RuntimeSetMemberState::Unavailable { reason: *reason }
                }
            };
            states.insert(clip.id().as_str().to_owned(), member_state);
            CollectionClipRecord::new(
                clip.id().as_str(),
                clip.source().as_str(),
                clip.take_index(),
                clip.take_name(),
                binding,
            )
        })
        .collect();
    (records, states)
}

fn bind_clip(clip: &CollectionClipV1, execution: &ExecutedSource) -> ClipBindingState {
    let ExecutedSource::Ready(source) = execution else {
        let ExecutedSource::Unavailable(reason) = execution else {
            unreachable!()
        };
        return ClipBindingState::Unavailable { reason: *reason };
    };
    let facts = source.loaded.source.source_facts();
    let rows = facts.clips();
    let Some(row) = rows.rows().get(clip.take_index() as usize) else {
        let reason = if rows.coverage().state() == SourceSetCoverageStateV1::Complete {
            ClipUnavailableReason::TakeIndexMissing
        } else {
            ClipUnavailableReason::TakeInventoryUnavailable
        };
        return ClipBindingState::Unavailable { reason };
    };
    let SourceObservationStateV1::Observed(observed_name) = row.source_name().state() else {
        return ClipBindingState::Unavailable {
            reason: ClipUnavailableReason::TakeNameUnavailable,
        };
    };
    if observed_name.as_str() != clip.take_name() {
        return ClipBindingState::Unavailable {
            reason: ClipUnavailableReason::TakeNameMismatched,
        };
    }
    let SourceObservationStateV1::Observed(normalized_index) = row.normalized_clip_index().state()
    else {
        return ClipBindingState::Unavailable {
            reason: ClipUnavailableReason::NormalizedClipUnavailable,
        };
    };
    let Some(measurements) = source.indexed_measurements.get(*normalized_index).cloned() else {
        return ClipBindingState::Unavailable {
            reason: ClipUnavailableReason::NormalizedClipUnavailable,
        };
    };
    if source.digest_mismatched {
        return ClipBindingState::Unavailable {
            reason: ClipUnavailableReason::DigestMismatched,
        };
    }
    let Some(normalized_name) = source
        .loaded
        .document()
        .clips
        .get(*normalized_index)
        .map(|clip| clip.name.clone())
    else {
        return ClipBindingState::Unavailable {
            reason: ClipUnavailableReason::NormalizedClipUnavailable,
        };
    };
    let check_reference = check_reference_for_normalized_name(
        clip.source().as_str(),
        *normalized_index,
        normalized_name,
        source.nested_output_available,
        &source.duplicate_normalized_names,
    );
    ClipBindingState::Established {
        observed_source_take_index: row.source_clip_index() as u32,
        observed_take_name: observed_name.as_str().to_owned(),
        normalized_clip_index: *normalized_index as u32,
        measurements: Box::new(measurements),
        check_reference,
    }
}

fn check_reference_for_normalized_name(
    source_key: &str,
    normalized_index: usize,
    normalized_name: String,
    nested_output_available: bool,
    duplicate_normalized_names: &BTreeSet<String>,
) -> CheckReferenceState {
    if !nested_output_available {
        CheckReferenceState::Unavailable {
            reason: CheckReferenceUnavailableReason::NestedOutputUnavailable,
        }
    } else if duplicate_normalized_names.contains(&normalized_name) {
        CheckReferenceState::Unavailable {
            reason: CheckReferenceUnavailableReason::DuplicateEmbeddedTakeName,
        }
    } else {
        CheckReferenceState::Available {
            reference: MeasurementReference::new(
                source_key,
                normalized_index as u32,
                normalized_name,
            ),
        }
    }
}

fn observed_takes(loaded: &LoadedInput) -> (TakeInventoryState, Vec<ObservedTake>) {
    let facts = loaded.source.source_facts();
    let inventory = if facts.clips().coverage().state() == SourceSetCoverageStateV1::Complete {
        TakeInventoryState::Complete
    } else {
        TakeInventoryState::Unavailable
    };
    let rows = facts
        .clips()
        .rows()
        .iter()
        .map(|row| {
            let name = match row.source_name().state() {
                SourceObservationStateV1::Observed(name) => TakeNameState::Available {
                    value: name.as_str().to_owned(),
                },
                SourceObservationStateV1::ProvenAbsent
                | SourceObservationStateV1::Unavailable(_) => TakeNameState::Unavailable,
            };
            let normalized = match row.normalized_clip_index().state() {
                SourceObservationStateV1::Observed(index) => loaded
                    .document()
                    .clips
                    .get(*index)
                    .and_then(|clip| {
                        u32::try_from(*index)
                            .ok()
                            .map(|index| NormalizedClipState::Available {
                                index,
                                name: clip.name.clone(),
                            })
                    })
                    .unwrap_or(NormalizedClipState::Unavailable),
                SourceObservationStateV1::ProvenAbsent
                | SourceObservationStateV1::Unavailable(_) => NormalizedClipState::Unavailable,
            };
            if let (
                TakeNameState::Available { value },
                NormalizedClipState::Available { index, name },
            ) = (&name, &normalized)
            {
                ObservedTake::new(row.source_clip_index() as u32, value, *index, name)
            } else {
                ObservedTake::with_unavailable(row.source_clip_index() as u32, name, normalized)
            }
        })
        .collect();
    (inventory, rows)
}

fn duplicate_clip_names(loaded: &LoadedInput) -> BTreeSet<String> {
    let mut names = BTreeMap::<&str, usize>::new();
    for clip in &loaded.document().clips {
        *names.entry(&clip.name).or_default() += 1;
    }
    names
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(name, _)| name.to_owned())
        .collect()
}

fn digest_state(
    pin: Option<&CollectionDigestPinV1>,
    input: Option<&InputIdentity>,
) -> DigestPinState {
    let Some(pin) = pin else {
        return DigestPinState::Unpinned;
    };
    match input {
        Some(input) if input.sha256() == pin.as_str() => DigestPinState::Matched {
            expected_sha256: pin.as_str().to_owned(),
        },
        input => DigestPinState::Mismatched {
            expected_sha256: pin.as_str().to_owned(),
            observed_sha256: input.map(|input| input.sha256().to_owned()),
        },
    }
}

fn prepare_config(resolution: CollectionConfigResolution) -> Result<PreparedConfig, String> {
    match resolution {
        CollectionConfigResolution::Default => Ok(PreparedConfig {
            loaded: LoadedConfig::without_file(),
            evidence: ConfigState::Default,
        }),
        CollectionConfigResolution::Explicit(path) => {
            let bytes = read_control_bounded(path.path(), COLLECTION_CONFIG_MAX_BYTES)
                .map_err(|_| "collection control error (config-read)".to_owned())?;
            let (config, declaration, transition_families) =
                parse_config(&bytes).map_err(|error| {
                    // Preserve collection lint's established encoding class,
                    // while still letting the shared strict reader inspect the
                    // raw bounded bytes before generic TOML decoding.
                    if error.starts_with(
                        "transition-family declaration control error (transition-family-encoding)",
                    ) {
                        "collection control error (config-encoding)".to_owned()
                    } else {
                        "collection control error (config-malformed)".to_owned()
                    }
                })?;
            config
                .validate()
                .map_err(|_| "collection control error (config-invalid)".to_owned())?;
            let engine = animsmith_engine::resolve_static(declaration)
                .map_err(|_| "collection control error (config-engine-invalid)".to_owned())?;
            let input = InputIdentity::from_bytes(&bytes);
            Ok(PreparedConfig {
                loaded: LoadedConfig {
                    config,
                    engine,
                    transition_families: Some(transition_families),
                    path: Some(PathBuf::from(path.declared())),
                    control_input: Some(path.path().to_path_buf()),
                    #[cfg(feature = "fbx")]
                    source: None,
                },
                evidence: ConfigState::Explicit {
                    locator: path.declared().to_owned(),
                    input,
                },
            })
        }
    }
}

/// Load a manifest-owned source configuration with the exact same default,
/// byte bound, and control-plane error mapping as collection lint.
pub(crate) fn load_collection_config_for_producer(
    resolution: CollectionConfigResolution,
) -> Result<LoadedConfig, String> {
    Ok(prepare_config(resolution)?.loaded)
}

fn read_control_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, ()> {
    let file = fs::File::open(path).map_err(|_| ())?;
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    if bytes.len() as u64 > limit {
        return Err(());
    }
    Ok(bytes)
}

fn read_primary_bounded(
    path: &Path,
    limit: u64,
) -> Result<Vec<u8>, (SourceUnavailableReason, u64)> {
    let file = fs::File::open(path).map_err(|error| {
        let reason = if error.kind() == std::io::ErrorKind::NotFound {
            SourceUnavailableReason::Missing
        } else {
            SourceUnavailableReason::Unreadable
        };
        (reason, 0)
    })?;
    let mut bytes = Vec::new();
    let read_result = file.take(limit.saturating_add(1)).read_to_end(&mut bytes);
    if read_result.is_err() {
        return Err((SourceUnavailableReason::Unreadable, bytes.len() as u64));
    }
    if bytes.len() as u64 > limit {
        let inspected = bytes.len() as u64;
        return Err((SourceUnavailableReason::TooLarge, inspected));
    }
    Ok(bytes)
}

fn classify_loader_error(
    _format: InputFormat,
    error: &super::InputLoadError,
) -> LoaderUnavailableReason {
    match error {
        super::InputLoadError::Gltf(animsmith_gltf::LoadError::Io { .. })
        | super::InputLoadError::Gltf(animsmith_gltf::LoadError::ExternalResource(_)) => {
            LoaderUnavailableReason::DependencyUnavailable
        }
        super::InputLoadError::Gltf(_) => LoaderUnavailableReason::MalformedInput,
        #[cfg(feature = "fbx")]
        super::InputLoadError::Fbx(_) => LoaderUnavailableReason::MalformedInput,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_reader_stops_at_n_plus_one() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("source.glb");
        fs::write(&path, b"12345").unwrap();
        assert_eq!(read_primary_bounded(&path, 5).unwrap(), b"12345");
        assert_eq!(
            read_primary_bounded(&path, 4).unwrap_err(),
            (SourceUnavailableReason::TooLarge, 5)
        );
        assert_eq!(
            read_primary_bounded(&path, 0).unwrap_err(),
            (SourceUnavailableReason::TooLarge, 1)
        );
    }

    #[test]
    fn aggregate_reader_stops_after_one_terminal_witness() {
        let cap = COLLECTION_OUTPUT_V2_MAX_AGGREGATE_SOURCE_BYTES;
        assert_eq!(next_source_limit(cap - 1), Some(1));
        assert_eq!(next_source_limit(cap), Some(0));
        assert_eq!(next_source_limit(cap + 1), None);
    }

    #[test]
    fn duplicate_normalized_key_has_no_name_addressed_reference() {
        let duplicates = BTreeSet::from(["Take 001".to_owned()]);
        assert!(matches!(
            check_reference_for_normalized_name(
                "source",
                1,
                "Take 001".to_owned(),
                true,
                &duplicates
            ),
            CheckReferenceState::Unavailable {
                reason: CheckReferenceUnavailableReason::DuplicateEmbeddedTakeName
            }
        ));
        assert!(matches!(
            check_reference_for_normalized_name(
                "source",
                1,
                "Take 001#1".to_owned(),
                false,
                &BTreeSet::new()
            ),
            CheckReferenceState::Unavailable {
                reason: CheckReferenceUnavailableReason::NestedOutputUnavailable
            }
        ));
    }

    #[test]
    fn gltf_dependency_failure_class_is_feature_independent() {
        let error = crate::InputLoadError::Gltf(animsmith_gltf::LoadError::ExternalResource(
            animsmith_gltf::ExternalResourceFailure::Unavailable,
        ));
        assert_eq!(
            classify_loader_error(InputFormat::Gltf, &error),
            LoaderUnavailableReason::DependencyUnavailable
        );
    }
}
