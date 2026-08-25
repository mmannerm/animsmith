//! Strict contact-fragment producer boundary for issue #152.
//!
//! The core contact-fragment contract owns the portable envelope and canonical
//! JSON. This module owns only CLI loading, exact selection, strict sampled
//! evidence, and one-file durable publication.

use super::collection_lint::load_collection_config_for_producer;
use super::collection_manifest::{
    CollectionPathResolver, CollectionSourceResolution, load_collection_manifest_with_identity,
};
use super::publish::{
    PublicationDestination, emit, emit_text, input_identity, publish_single,
    require_external_dependencies_safe_for_publication, require_writable_destination,
};
use super::{Format, LoadedConfig, LoadedInput, load_with_config_for_producer};
use crate::producer::{self, Command, Failure, Kind, Stage};
use animsmith_core::{
    ContactClipReferenceV1, ContactEventKindV1, ContactEventV1, ContactEventWindowV1,
    ContactExtensionV1, ContactFragmentV1, ContactPhaseV1, ContactProducerV1, ContactRoleV1,
    DependencyClosureV1, MetricGrids, Role, SourceClipFactV1, SourceFactSetV1,
    SourceObservationStateV1, SourceSetCoverageStateV1, StanceSideV1, ToolInfo,
    resolve_configured_roles, resolve_stance_support_v1, validate_document_shape,
};
use serde_json::json;
use std::path::Path;
use std::process::ExitCode;

const DETECTOR_EXTENSION: &str = "urn:animsmith:contact-support-detector:1";
const MAX_FRAMES: usize = 1_000_000;
const MAX_RETAINED_RUNS: usize = 2_048;

/// An exact selection bound both to the serialized clip reference and to the
/// normalized document index whose sampled evidence it authorizes.
struct ClipSelection {
    reference: ContactClipReferenceV1,
    normalized_index: usize,
}

/// Produce a direct document-scoped contact fragment.
pub(crate) fn run_direct(
    input: &Path,
    requested_clip: &str,
    output: &Path,
    format: Format,
    tool: ToolInfo,
    config: &LoadedConfig,
) -> Result<ExitCode, String> {
    let mut consumed = vec![("input", input)];
    if let Some(path) = config.control_input() {
        consumed.push(("config", path));
    }
    require_output_distinct_from_consumed(output, &consumed)?;
    let loaded = match load_with_config_for_producer(input, config) {
        Ok(loaded) => loaded,
        Err(Failure::Refusal(rejection)) => return emit_refusal(format, tool, rejection),
        Err(Failure::Operator(error)) => return Err(error),
    };
    let selection = match exact_document_clip(&loaded, requested_clip) {
        Ok(witness) => witness,
        Err(rejection) => return emit_refusal(format, tool, rejection),
    };
    publish_loaded(&loaded, input, config, selection, output, format, tool)
}

/// Produce a collection-scoped contact fragment by reloading the witnessed
/// source; collection-output evidence is intentionally not an input to this path.
pub(crate) fn run_collection(
    manifest_path: &Path,
    requested_clip: &str,
    output: &Path,
    format: Format,
    tool: ToolInfo,
) -> Result<ExitCode, String> {
    let loaded_manifest =
        load_collection_manifest_with_identity(manifest_path).map_err(|error| error.to_string())?;
    let manifest = loaded_manifest.manifest;
    let selected = manifest
        .clips()
        .iter()
        .filter(|clip| clip.id().as_str() == requested_clip)
        .collect::<Vec<_>>();
    let [selected] = selected.as_slice() else {
        return emit_refusal(
            format,
            tool,
            selection_refusal("collection logical clip selection is not exact and unique"),
        );
    };
    let resolver = CollectionPathResolver::new(manifest_path, manifest.input_root())
        .map_err(|error| error.to_string())?;
    let resolutions = resolver
        .resolve_sources(manifest.sources())
        .map_err(|error| error.to_string())?;
    let source = manifest
        .sources()
        .iter()
        .find(|source| source.key() == selected.source())
        .ok_or_else(|| "collection control error (selected-source-missing)".to_owned())?;
    let resolution = resolutions.get(source.key().as_str()).ok_or_else(|| {
        "collection control error (selected-source-resolution-missing)".to_owned()
    })?;
    let CollectionSourceResolution::Ready(source_path) = resolution else {
        return emit_refusal(
            format,
            tool,
            incomplete_refusal("collection source is unavailable"),
        );
    };
    let config_resolution = resolver
        .resolve_config(source.config())
        .map_err(|error| error.to_string())?;
    let config = load_collection_config_for_producer(config_resolution)?;
    let mut consumed = vec![("manifest", manifest_path), ("source", source_path.path())];
    if let Some(path) = config.control_input() {
        consumed.push(("config", path));
    }
    require_output_distinct_from_consumed(output, &consumed)?;
    let loaded = match load_with_config_for_producer(source_path.path(), &config) {
        Ok(loaded) => loaded,
        Err(Failure::Refusal(rejection)) => return emit_refusal(format, tool, rejection),
        Err(Failure::Operator(error)) => return Err(error),
    };
    let selection = match exact_collection_clip(&loaded, selected) {
        Ok(selection) => selection,
        Err(rejection) => return emit_refusal(format, tool, rejection),
    };
    publish_loaded(
        &loaded,
        source_path.path(),
        &config,
        selection,
        output,
        format,
        tool,
    )
}

fn require_output_distinct_from_consumed(
    output: &Path,
    consumed: &[(&str, &Path)],
) -> Result<(), String> {
    let output_destination = PublicationDestination::new("output", output)?;
    for (label, input) in consumed {
        let identity = input_identity(input)?;
        if identity == output_destination.identity() {
            return Err(format!(
                "contact-fragment {label} and output must be different paths, but both resolve to {}",
                identity.display()
            ));
        }
    }
    require_writable_destination(output)
}

fn require_source_dependencies_safe(
    input: &Path,
    loaded: &LoadedInput,
    output: &Path,
) -> Result<(), String> {
    require_external_dependencies_safe_for_publication(
        "contact-fragment",
        super::publish::parent_or_current(input),
        loaded.dependency_closure(),
        &[("output", output)],
    )
}

fn exact_document_clip(
    loaded: &LoadedInput,
    requested_clip: &str,
) -> Result<ClipSelection, producer::Rejection> {
    // Document clip names may be synthesized to keep an internal document
    // addressable (for example `walk#1` after a duplicate glTF animation
    // name). The CLI promise is stricter: it selects one exact *authored*
    // take name, so establish uniqueness from the raw source rows instead.
    let source_clips = loaded.source_facts().clips();
    if source_clips.coverage().state() != SourceSetCoverageStateV1::Complete {
        return Err(selection_refusal(
            "document clip inventory is not complete enough for exact selection",
        ));
    }
    let selected = source_clips
        .rows()
        .iter()
        .filter(|clip| {
            matches!(
                clip.source_name().state(),
                SourceObservationStateV1::Observed(name) if name.as_str() == requested_clip
            )
        })
        .collect::<Vec<_>>();
    let [selected] = selected.as_slice() else {
        return Err(selection_refusal(
            "document clip selection is not exact and unique",
        ));
    };
    let SourceObservationStateV1::Observed(index) = selected.normalized_clip_index().state() else {
        return Err(selection_refusal(
            "document clip selection has no normalized take witness",
        ));
    };
    if loaded
        .document()
        .clips
        .get(*index)
        .is_none_or(|clip| clip.name != requested_clip)
    {
        return Err(selection_refusal(
            "document clip selection does not match the reloaded take witness",
        ));
    }
    Ok(ClipSelection {
        reference: ContactClipReferenceV1::document(requested_clip)
            .map_err(|_| selection_refusal("document clip name cannot be represented"))?,
        normalized_index: *index,
    })
}

/// Resolve a manifest's source-domain take witness to the loader's normalized
/// document index. `take_index` names a raw source row, never an incidental
/// position in `Document::clips`.
fn exact_collection_clip(
    loaded: &LoadedInput,
    requested: &animsmith_core::CollectionClipV1,
) -> Result<ClipSelection, producer::Rejection> {
    let source_clips = loaded.source_facts().clips();
    let normalized_index = resolve_collection_take_witness(
        source_clips,
        loaded.document().clips.len(),
        requested.take_index() as usize,
        requested.take_name(),
    )?;
    Ok(ClipSelection {
        reference: ContactClipReferenceV1::collection(
            requested.id().as_str(),
            requested.source().as_str(),
            requested.take_index(),
            requested.take_name(),
        )
        .map_err(|_| selection_refusal("collection clip witness cannot be represented"))?,
        normalized_index,
    })
}

/// Resolve one collection raw-take selector through complete source facts.
/// This is deliberately independent of `Document::clips` names: loaders may
/// synthesize those names while retaining the source index/name witness.
fn resolve_collection_take_witness(
    source_clips: &SourceFactSetV1<SourceClipFactV1>,
    document_clip_count: usize,
    raw_take_index: usize,
    take_name: &str,
) -> Result<usize, producer::Rejection> {
    if source_clips.coverage().state() != SourceSetCoverageStateV1::Complete {
        return Err(selection_refusal(
            "collection take inventory is not complete enough for exact selection",
        ));
    }
    let Some(row) = source_clips.rows().get(raw_take_index) else {
        return Err(selection_refusal(
            "collection take index is absent from the reloaded source",
        ));
    };
    let SourceObservationStateV1::Observed(observed_name) = row.source_name().state() else {
        return Err(selection_refusal(
            "collection take name is unavailable from the reloaded source",
        ));
    };
    if observed_name.as_str() != take_name {
        return Err(selection_refusal(
            "collection take name does not match the reloaded source index",
        ));
    }
    let SourceObservationStateV1::Observed(normalized_index) = row.normalized_clip_index().state()
    else {
        return Err(selection_refusal(
            "collection take selection has no normalized take witness",
        ));
    };
    if *normalized_index >= document_clip_count {
        return Err(selection_refusal(
            "collection normalized take witness is absent from the reloaded source",
        ));
    }
    Ok(*normalized_index)
}

fn publish_loaded(
    loaded: &LoadedInput,
    source_input: &Path,
    config: &LoadedConfig,
    selection: ClipSelection,
    output: &Path,
    format: Format,
    tool: ToolInfo,
) -> Result<ExitCode, String> {
    let fragment = match build_fragment(loaded, config, selection) {
        Ok(fragment) => fragment,
        Err(rejection) => return emit_refusal(format, tool, rejection),
    };
    require_source_dependencies_safe(source_input, loaded, output)?;
    let bytes = fragment
        .canonical_json()
        .map_err(|error| format!("cannot serialize contact fragment: {error}"))?;
    let temp = tempfile::Builder::new()
        .prefix(".animsmith-contact-fragment-")
        .tempfile_in(super::publish::parent_or_current(output))
        .map_err(|error| format!("cannot create temporary contact fragment: {error}"))?
        .into_temp_path();
    std::fs::write(&temp, &bytes)
        .map_err(|error| format!("cannot write temporary contact fragment: {error}"))?;
    publish_single(&temp, output)?;
    match format {
        Format::Json => emit(&bytes),
        Format::Text => emit_text(&format!(
            "published contact fragment: {} event(s) -> {}\n",
            fragment.events().len(),
            output.display()
        )),
    }
    Ok(ExitCode::SUCCESS)
}

fn build_fragment(
    loaded: &LoadedInput,
    config: &LoadedConfig,
    selection: ClipSelection,
) -> Result<ContactFragmentV1, producer::Rejection> {
    let document = loaded.document();
    validate_document_shape(document).map_err(|_| {
        incomplete_refusal("source document shape is not valid for strict contact evidence")
    })?;
    let closure = loaded.dependency_closure();
    let closure_identity = complete_closure(closure, loaded.input())?;
    let selected_index = selection.normalized_index;
    let clip = document
        .clips
        .get(selected_index)
        .ok_or_else(|| selection_refusal("selected collection clip disappeared"))?;
    if !clip.duration_s.is_finite() || clip.duration_s <= 0.0 {
        return Err(incomplete_refusal(
            "clip duration is not finite and positive",
        ));
    }
    let frame_count = animsmith_core::metrics::metric_frame_count(clip)
        .ok_or_else(|| incomplete_refusal("metric grid is unavailable"))?;
    validate_frame_count(frame_count)?;
    let grids = MetricGrids::new(document);
    let grid = grids
        .grid(selected_index)
        .ok_or_else(|| incomplete_refusal("metric grid is unavailable"))?;
    if grid.frame_count() < 3 || grid.times.len() != grid.frame_count() {
        return Err(incomplete_refusal("metric grid is incomplete"));
    }
    if grid.times.iter().any(|time| !time.is_finite()) {
        return Err(incomplete_refusal(
            "metric grid has a non-finite sample time",
        ));
    }
    let roles = resolve_configured_roles(&document.skeleton, &config.config.rig);
    let contact_height_m = config
        .config
        .check_settings("foot-slide")
        .contact_height_m
        // This is the frozen `foot-slide` default. The producer intentionally
        // reads the same config key without changing that check's semantics.
        .unwrap_or(0.03);
    if !contact_height_m.is_finite() || contact_height_m < 0.0 {
        return Err(incomplete_refusal(
            "contact height is not finite and non-negative",
        ));
    }
    let mut events = Vec::new();
    let mut retained_runs = 0usize;
    let mut extension_roles = serde_json::Map::new();
    for side in [StanceSideV1::Left, StanceSideV1::Right] {
        let stance = resolve_stance_support_v1(&grid, &roles, side, contact_height_m)
            .ok_or_else(|| incomplete_refusal("bilateral foot/toe role evidence is incomplete"))?;
        let role = contact_role(stance.role())?;
        let role_name = stance.role().as_str();
        let side_name = match side {
            StanceSideV1::Left => "left",
            StanceSideV1::Right => "right",
        };
        extension_roles.insert(side_name.to_owned(), json!(role_name));
        for frame in 0..grid.frame_count() {
            let position = grid.model_position(frame, stance.bone());
            if !position.is_finite() {
                return Err(incomplete_refusal(
                    "bilateral stance samples must be finite",
                ));
            }
        }
        for run in stance.retained_runs() {
            reserve_retained_run(&mut retained_runs)?;
            let minimum_frame =
                earliest_minimum_frame(&grid, stance.bone(), run.start_frame, run.end_frame);
            let start = normalized_frame(run.start_frame, grid.frame_count())?;
            let end = normalized_frame(run.end_frame, grid.frame_count())?;
            let marker = normalized_frame(minimum_frame, grid.frame_count())?;
            events.push(
                ContactEventV1::window(
                    format!("support/{role_name}/{}-{}", run.start_frame, run.end_frame),
                    role,
                    ContactPhaseV1::Begin,
                    ContactEventWindowV1::new(start, end)
                        .map_err(|_| incomplete_refusal("support window is invalid"))?,
                    None,
                )
                .map_err(|_| incomplete_refusal("support event is invalid"))?,
            );
            events.push(
                ContactEventV1::point(
                    format!("marker/{role_name}/{minimum_frame}"),
                    role,
                    ContactPhaseV1::Marker,
                    marker,
                    None,
                )
                .map_err(|_| incomplete_refusal("support marker is invalid"))?,
            );
        }
    }
    let extension = ContactExtensionV1::new(
        DETECTOR_EXTENSION,
        1,
        json!({
            "algorithm": "stance-support-v1",
            "sampling": "metric-grid-longest-authored-channel",
            "max_frames": MAX_FRAMES,
            "contact_height_m": contact_height_m,
            "roles": extension_roles,
        }),
    )
    .map_err(|_| incomplete_refusal("strict detector extension is invalid"))?;
    validate_contact_event_relationship(&events)?;
    ContactFragmentV1::new(
        ContactProducerV1::new("animsmith", env!("CARGO_PKG_VERSION"))
            .map_err(|_| incomplete_refusal("producer identity is invalid"))?,
        loaded.input().clone(),
        closure_identity,
        selection.reference,
        clip.duration_s,
        events,
        vec![extension],
    )
    .map_err(|_| incomplete_refusal("contact fragment cannot represent strict evidence"))
}

fn reserve_retained_run(retained_runs: &mut usize) -> Result<(), producer::Rejection> {
    if *retained_runs == MAX_RETAINED_RUNS {
        return Err(incomplete_refusal(
            "retained support runs exceed the 2048-run limit",
        ));
    }
    *retained_runs += 1;
    Ok(())
}

/// Verify the detector's support-window contract from typed role/time/kind
/// facts. Event identifiers remain opaque labels and are never parsed.
fn validate_contact_event_relationship(
    events: &[ContactEventV1],
) -> Result<(), producer::Rejection> {
    let mut windows: Vec<(ContactRoleV1, ContactEventWindowV1)> = Vec::new();
    let mut markers: Vec<(ContactRoleV1, f64)> = Vec::new();
    for event in events {
        match event.kind() {
            ContactEventKindV1::Window(window) if event.phase() == ContactPhaseV1::Begin => {
                if windows.iter().any(|(previous_role, previous)| {
                    *previous_role == event.role() && window.start() <= previous.end()
                }) {
                    return Err(incomplete_refusal(
                        "same-role support windows are not ordered and non-overlapping",
                    ));
                }
                windows.push((event.role(), window));
            }
            ContactEventKindV1::Point(time) if event.phase() == ContactPhaseV1::Marker => {
                markers.push((event.role(), time));
            }
            ContactEventKindV1::Window(_) | ContactEventKindV1::Point(_) => {
                return Err(incomplete_refusal(
                    "support evidence contains an unexpected event kind or phase",
                ));
            }
        }
    }
    for (role, window) in &windows {
        if markers
            .iter()
            .filter(|(marker_role, time)| {
                *marker_role == *role && window.start() <= *time && *time <= window.end()
            })
            .count()
            != 1
        {
            return Err(incomplete_refusal(
                "each support window must contain exactly one same-role marker",
            ));
        }
    }
    for (role, time) in markers {
        if windows
            .iter()
            .filter(|(window_role, window)| {
                *window_role == role && window.start() <= time && time <= window.end()
            })
            .count()
            != 1
        {
            return Err(incomplete_refusal(
                "each support marker must belong to exactly one same-role window",
            ));
        }
    }
    Ok(())
}

fn complete_closure(
    closure: &DependencyClosureV1,
    primary: &animsmith_core::InputIdentity,
) -> Result<animsmith_core::DependencyClosureIdentityV1, producer::Rejection> {
    validate_complete_closure_binding(
        closure.primary_input(),
        closure.coverage().is_complete(),
        closure.identity(),
        primary,
    )
}

fn validate_complete_closure_binding(
    closure_primary: &animsmith_core::InputIdentity,
    coverage_complete: bool,
    identity: Option<&animsmith_core::DependencyClosureIdentityV1>,
    primary: &animsmith_core::InputIdentity,
) -> Result<animsmith_core::DependencyClosureIdentityV1, producer::Rejection> {
    if closure_primary != primary {
        return Err(incomplete_refusal(
            "dependency closure primary does not equal source artifact",
        ));
    }
    if !coverage_complete {
        return Err(incomplete_refusal("dependency closure is not complete"));
    }
    identity
        .cloned()
        .ok_or_else(|| incomplete_refusal("complete dependency closure has no identity"))
}

fn validate_frame_count(frame_count: usize) -> Result<(), producer::Rejection> {
    if frame_count > MAX_FRAMES {
        return Err(incomplete_refusal(
            "metric grid exceeds the 1000000-frame limit",
        ));
    }
    Ok(())
}

fn contact_role(role: Role) -> Result<ContactRoleV1, producer::Rejection> {
    match role {
        Role::LeftFoot => Ok(ContactRoleV1::LeftFoot),
        Role::RightFoot => Ok(ContactRoleV1::RightFoot),
        Role::LeftToe => Ok(ContactRoleV1::LeftToe),
        Role::RightToe => Ok(ContactRoleV1::RightToe),
        _ => Err(incomplete_refusal(
            "stance support selected a non-foot role",
        )),
    }
}

fn earliest_minimum_frame(
    grid: &animsmith_core::PoseGrid,
    bone: usize,
    start: usize,
    end: usize,
) -> usize {
    let mut minimum = start;
    for frame in (start + 1)..=end {
        if grid.model_position(frame, bone).y < grid.model_position(minimum, bone).y {
            minimum = frame;
        }
    }
    minimum
}

fn normalized_frame(frame: usize, count: usize) -> Result<f64, producer::Rejection> {
    let intervals = count
        .checked_sub(1)
        .filter(|intervals| *intervals > 0)
        .ok_or_else(|| incomplete_refusal("metric grid has no intervals"))?;
    Ok(frame as f64 / intervals as f64)
}

fn selection_refusal(detail: &'static str) -> producer::Rejection {
    producer::Rejection::new(Stage::Selection, Kind::SelectionMismatch, detail)
}

fn incomplete_refusal(detail: &'static str) -> producer::Rejection {
    producer::Rejection::new(Stage::Analysis, Kind::IncompleteEvidence, detail)
}

fn emit_refusal(
    format: Format,
    tool: ToolInfo,
    rejection: producer::Rejection,
) -> Result<ExitCode, String> {
    let mut delivery = producer::ProcessRefusalDelivery;
    producer::emit_rejection(
        Command::ContactFragment,
        format,
        tool,
        rejection,
        &mut delivery,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use animsmith_core::{
        DependencyClosureBuilderV1, InputIdentity, SourceChannelFactV1, SourceLoaderDispositionV1,
        SourceObservationV1, SourceProvenanceV1, SourceSetCoverageV1, SourceTextV1,
        SourceTimeRangeV1,
    };

    fn window(start: f64, end: f64) -> ContactEventV1 {
        ContactEventV1::window(
            "opaque-window",
            ContactRoleV1::LeftFoot,
            ContactPhaseV1::Begin,
            ContactEventWindowV1::new(start, end).unwrap(),
            None,
        )
        .unwrap()
    }

    fn marker(time: f64) -> ContactEventV1 {
        ContactEventV1::point(
            "opaque-marker",
            ContactRoleV1::LeftFoot,
            ContactPhaseV1::Marker,
            time,
            None,
        )
        .unwrap()
    }

    #[test]
    fn contact_event_relationship_uses_typed_role_time_and_kind() {
        let valid = vec![window(0.0, 0.25), marker(0.125)];
        assert!(validate_contact_event_relationship(&valid).is_ok());

        let missing_marker = vec![window(0.0, 0.25)];
        assert!(validate_contact_event_relationship(&missing_marker).is_err());

        // These are non-overlapping, but not in production order. The
        // identifiers deliberately give no ordering signal.
        let out_of_order = vec![
            ContactEventV1::window(
                "unrelated-a",
                ContactRoleV1::LeftFoot,
                ContactPhaseV1::Begin,
                ContactEventWindowV1::new(0.75, 1.0).unwrap(),
                None,
            )
            .unwrap(),
            ContactEventV1::point(
                "unrelated-b",
                ContactRoleV1::LeftFoot,
                ContactPhaseV1::Marker,
                0.875,
                None,
            )
            .unwrap(),
            ContactEventV1::window(
                "unrelated-c",
                ContactRoleV1::LeftFoot,
                ContactPhaseV1::Begin,
                ContactEventWindowV1::new(0.0, 0.25).unwrap(),
                None,
            )
            .unwrap(),
            ContactEventV1::point(
                "unrelated-d",
                ContactRoleV1::LeftFoot,
                ContactPhaseV1::Marker,
                0.125,
                None,
            )
            .unwrap(),
        ];
        assert!(validate_contact_event_relationship(&out_of_order).is_err());
    }

    #[test]
    fn retained_run_cap_accepts_2048_and_refuses_2049() {
        let mut retained = 0;
        for _ in 0..MAX_RETAINED_RUNS {
            reserve_retained_run(&mut retained).unwrap();
        }
        assert_eq!(retained, MAX_RETAINED_RUNS);
        assert!(reserve_retained_run(&mut retained).is_err());
    }

    #[test]
    fn complete_closure_requires_primary_complete_coverage_and_identity() {
        let primary = InputIdentity::from_bytes(b"primary");
        let closure =
            DependencyClosureBuilderV1::new(primary.clone(), SourceSetCoverageV1::complete(), 0)
                .finish()
                .unwrap();

        assert!(complete_closure(&closure, &primary).is_ok());
        assert!(complete_closure(&closure, &InputIdentity::from_bytes(b"other")).is_err());
        assert!(
            complete_closure(&DependencyClosureV1::unavailable(primary.clone()), &primary,)
                .is_err()
        );
        assert!(validate_complete_closure_binding(&primary, true, None, &primary).is_err());
    }

    #[test]
    fn collection_take_witness_uses_normalized_index_not_raw_index() {
        let provenance = SourceProvenanceV1::format_defined();
        let source_clips = SourceFactSetV1::complete(vec![SourceClipFactV1::new(
            0,
            SourceObservationV1::observed(
                SourceTextV1::new("raw-take").unwrap(),
                provenance.clone(),
                SourceLoaderDispositionV1::Preserved,
            ),
            SourceObservationV1::observed(
                2,
                provenance.clone(),
                SourceLoaderDispositionV1::Normalized,
            ),
            SourceObservationV1::<SourceTimeRangeV1>::proven_absent(provenance.clone()),
            SourceObservationV1::<SourceTimeRangeV1>::proven_absent(provenance),
            SourceFactSetV1::<SourceChannelFactV1>::complete(Vec::new()),
        )]);

        assert_eq!(
            resolve_collection_take_witness(&source_clips, 3, 0, "raw-take").unwrap(),
            2
        );
    }

    #[test]
    fn frame_cap_accepts_one_million_and_refuses_next_frame() {
        assert!(validate_frame_count(MAX_FRAMES).is_ok());
        assert!(validate_frame_count(MAX_FRAMES + 1).is_err());
    }
}
