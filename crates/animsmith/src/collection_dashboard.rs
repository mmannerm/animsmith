//! Current-state collection dashboard publication.
//!
//! The existing collection-output V11 reader remains the only authority for
//! source, clip, and runtime-set facts. This module validates that input,
//! projects a deliberately presentation-oriented V1 authority, and publishes
//! that JSON beside an offline HTML rendering of the same bytes.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Cursor, Read, Write};
use std::path::Path;
use std::process::ExitCode;

use animsmith_core::InputIdentity;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{collection_output, publish, render};

const COLLECTION_DASHBOARD_V1_ID: &str = "urn:animsmith:schema:collection-dashboard:1";
const COLLECTION_DASHBOARD_V1_VERSION: u32 = 1;
const TRANSITION_POSE_V1_ID: &str = "urn:animsmith:schema:transition-pose-evaluation:1";
const MAX_EVALUATION_BYTES: u64 = 8 * 1024 * 1024;
const MAX_DASHBOARD_BYTES: u64 = 16 * 1024 * 1024;

/// Render and atomically publish one dashboard HTML/evidence pair.
pub(crate) fn run(
    collection_path: &Path,
    output: &Path,
    authority: &Path,
    evaluation_path: Option<&Path>,
    report_links: &[String],
) -> Result<ExitCode, String> {
    ensure_distinct_paths(collection_path, output, authority, evaluation_path)?;
    let collection_bytes = read_bounded(
        collection_path,
        collection_output::COLLECTION_OUTPUT_MAX_SERIALIZED_BYTES,
        "collection output",
    )?;
    let collection =
        collection_output::read_current_collection_output(Cursor::new(&collection_bytes))
            .map_err(|error| format!("invalid current collection output: {error}"))?;
    let collection = collection
        .dashboard_input()
        .map_err(|error| format!("invalid current collection output: {error}"))?;
    let report_links = parse_report_links(report_links, &collection)?;
    let evaluation = match evaluation_path {
        Some(path) => Some(read_compatible_evaluation(path, &collection)?),
        None => None,
    };
    let authority_value = build_authority(
        InputIdentity::from_bytes(&collection_bytes),
        &collection,
        evaluation,
        report_links,
    )?;
    let authority_bytes = serialize_authority_bounded(&authority_value)?;
    validate_authority_readback(&authority_bytes)?;
    let authority_text = std::str::from_utf8(&authority_bytes)
        .map_err(|_| "dashboard authority serialization is not UTF-8".to_owned())?;
    let html = animsmith_report::render_collection_dashboard(authority_text);
    let html_bytes = html.into_bytes();
    if html_bytes.len() as u64 > MAX_DASHBOARD_BYTES {
        return Err("collection dashboard exceeds its bounded output limit".to_owned());
    }
    let artifact_temp = stage(output, &html_bytes, "collection-dashboard-html")?;
    let evidence_temp = stage(authority, &authority_bytes, "collection-dashboard-json")?;
    publish::publish_pair(
        artifact_temp.as_ref(),
        output,
        evidence_temp.as_ref(),
        authority,
        false,
    )?;
    publish::emit_text(&render::render_report_written(
        output,
        collection.clips.len(),
        0,
        html_bytes.len(),
    ));
    Ok(ExitCode::SUCCESS)
}

fn serialize_authority_bounded(
    authority: &CollectionDashboardAuthorityV1,
) -> Result<Vec<u8>, String> {
    let mut counter = BoundedWriter::new(MAX_DASHBOARD_BYTES);
    serde_json::to_writer(&mut counter, authority)
        .map_err(|_| "collection dashboard exceeds its bounded output limit".to_owned())?;
    Ok(counter.into_bytes())
}

fn validate_authority_readback(bytes: &[u8]) -> Result<(), String> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| "dashboard authority serialization is invalid JSON".to_owned())?;
    let schema: Value = serde_json::from_str(include_str!(
        "../../../docs/schemas/collection-dashboard-v1.schema.json"
    ))
    .map_err(|_| "dashboard authority schema is invalid".to_owned())?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|_| "dashboard authority schema cannot compile".to_owned())?;
    if validator.iter_errors(&value).next().is_some() {
        return Err("dashboard authority fails its V1 schema".to_owned());
    }
    let readback = serde_json::from_slice::<DashboardAuthorityReadback>(bytes)
        .map_err(|_| "dashboard authority fails typed V1 readback".to_owned())?;
    readback.validate_semantics()?;
    Ok(())
}

struct BoundedWriter {
    bytes: u64,
    limit: u64,
    output: Vec<u8>,
}
impl BoundedWriter {
    fn new(limit: u64) -> Self {
        Self {
            bytes: 0,
            limit,
            output: Vec::new(),
        }
    }
    fn into_bytes(self) -> Vec<u8> {
        self.output
    }
}
impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| std::io::Error::other("dashboard limit"))?;
        if self.bytes > self.limit {
            return Err(std::io::Error::other("dashboard limit"));
        }
        self.output.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn read_bounded(path: &Path, limit: u64, label: &str) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|_| format!("cannot read {label}"))?;
    let mut reader = file.take(limit.saturating_add(1));
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|_| format!("cannot read {label}"))?;
    if bytes.len() as u64 > limit {
        return Err(format!("{label} exceeds its bounded reader limit"));
    }
    Ok(bytes)
}

fn ensure_distinct_paths(
    collection: &Path,
    output: &Path,
    authority: &Path,
    evaluation: Option<&Path>,
) -> Result<(), String> {
    let mut paths = vec![
        ("collection input", collection),
        ("dashboard output", output),
        ("dashboard authority", authority),
    ];
    if let Some(evaluation) = evaluation {
        paths.push(("evaluation input", evaluation));
    }
    let resolved = paths
        .iter()
        .map(|(name, path)| Ok((*name, resolved_path(path)?)))
        .collect::<Result<Vec<_>, String>>()?;
    for (index, (left_name, left)) in resolved.iter().enumerate() {
        for (right_name, right) in resolved.iter().skip(index + 1) {
            if left == right {
                return Err(format!("{left_name} and {right_name} paths must differ"));
            }
        }
    }
    Ok(())
}

fn resolved_path(path: &Path) -> Result<std::path::PathBuf, String> {
    if path.exists() {
        return std::fs::canonicalize(path)
            .map_err(|_| format!("cannot resolve {}", path.display()));
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| format!("invalid path {}", path.display()))?;
    let parent = std::fs::canonicalize(parent)
        .map_err(|_| format!("cannot resolve {}", parent.display()))?;
    Ok(parent.join(name))
}

fn stage(destination: &Path, bytes: &[u8], label: &str) -> Result<tempfile::TempPath, String> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::Builder::new()
        .prefix(&format!(".animsmith-{label}-"))
        .tempfile_in(parent)
        .map_err(|error| format!("cannot stage {}: {error}", destination.display()))?;
    temporary
        .write_all(bytes)
        .map_err(|error| format!("cannot stage {}: {error}", destination.display()))?;
    temporary
        .flush()
        .map_err(|error| format!("cannot stage {}: {error}", destination.display()))?;
    Ok(temporary.into_temp_path())
}

fn parse_report_links(
    entries: &[String],
    collection: &collection_output::CollectionDashboardInput,
) -> Result<BTreeMap<String, String>, String> {
    let declared = collection
        .clips
        .iter()
        .map(|clip| clip.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut links = BTreeMap::new();
    for entry in entries {
        let (id, reference) = entry
            .split_once('=')
            .ok_or_else(|| "--asset-report must be LOGICAL_ID=RELATIVE_PATH".to_owned())?;
        if !declared.contains(id) {
            return Err(format!(
                "asset report link names undeclared logical clip {id:?}"
            ));
        }
        if !safe_relative_report_reference(reference) {
            return Err(format!(
                "asset report link for {id:?} is not a safe relative reference"
            ));
        }
        if links.insert(id.to_owned(), reference.to_owned()).is_some() {
            return Err(format!(
                "asset report link is duplicated for logical clip {id:?}"
            ));
        }
    }
    Ok(links)
}

fn safe_relative_report_reference(reference: &str) -> bool {
    !reference.is_empty()
        && reference.len() <= 4096
        && !reference.starts_with('/')
        && !reference.starts_with('\\')
        && !reference.contains('\\')
        && !reference.contains('#')
        && !reference.contains('?')
        && !reference.contains(':')
        // Percent decoding can turn a harmless-looking path segment into an
        // escaping `..` or a scheme delimiter in browser URL resolution.
        && !reference.contains('%')
        && !reference.chars().any(char::is_control)
        && reference
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn read_compatible_evaluation(
    path: &Path,
    collection: &collection_output::CollectionDashboardInput,
) -> Result<EvaluationAuthorityV1, String> {
    let bytes = read_bounded(path, MAX_EVALUATION_BYTES, "transition-pose evaluation")?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| "transition-pose evaluation JSON is invalid".to_owned())?;
    let schema: Value = serde_json::from_str(include_str!(
        "../../../docs/schemas/transition-pose-evaluation-v1.schema.json"
    ))
    .map_err(|_| "transition-pose evaluation schema is invalid".to_owned())?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|_| "transition-pose evaluation schema cannot compile".to_owned())?;
    if validator.iter_errors(&value).next().is_some() {
        return Err("transition-pose evaluation violates its strict V1 schema".to_owned());
    }
    let wire: TransitionPoseEvaluationWire = serde_json::from_value(value)
        .map_err(|_| "transition-pose evaluation has an unsupported V1 shape".to_owned())?;
    if wire.schema != TRANSITION_POSE_V1_ID
        || wire.schema_version != 1
        || wire.subject_input != IdentityWire::from_input_identity(&collection.manifest)
    {
        return Err(
            "transition-pose evaluation is not compatible with this collection output".to_owned(),
        );
    }
    Ok(EvaluationAuthorityV1 {
        input: InputIdentity::from_bytes(&bytes),
        status: wire.status,
        decision: wire.decision,
        reason: wire.reason,
        families: wire
            .families
            .into_iter()
            .map(|family| EvaluationFamilyV1 {
                id: family.family_id,
                status: family.status,
                decision: family.decision,
                reason: family.reason,
                members: family
                    .members
                    .into_iter()
                    .map(|member| {
                        let logical_clip = member.source_input.as_ref().and_then(|input| {
                            let candidates = collection
                                .clips
                                .iter()
                                .filter(|clip| {
                                    clip.take_index as u64 == member.take_index
                                        && clip.take_name == member.take_name
                                        && collection.sources.iter().any(|source| {
                                            source.key == clip.source
                                                && source.input.as_ref().is_some_and(
                                                    |source_input| {
                                                        input
                                                            == &IdentityWire::from_input_identity(
                                                                source_input,
                                                            )
                                                    },
                                                )
                                        })
                                })
                                .map(|clip| clip.id.clone())
                                .collect::<Vec<_>>();
                            (candidates.len() == 1).then(|| candidates[0].clone())
                        });
                        EvaluationMemberV1 {
                            take_index: member.take_index,
                            take_name: member.take_name,
                            source_input: member.source_input,
                            logical_clip,
                        }
                    })
                    .collect(),
                pair_findings: family
                    .pairs
                    .into_iter()
                    .filter_map(|pair| {
                        (!pair.translation_offenders.is_empty()
                            || !pair.rotation_offenders.is_empty())
                        .then_some(EvaluationPairFindingV1 {
                            member_indices: pair.member_indices,
                            boundary: pair.boundary,
                            translation_offenders: pair.translation_offenders,
                            rotation_offenders: pair.rotation_offenders,
                        })
                    })
                    .collect(),
            })
            .collect(),
    })
}

fn build_authority(
    collection_output: InputIdentity,
    collection: &collection_output::CollectionDashboardInput,
    evaluation: Option<EvaluationAuthorityV1>,
    report_links: BTreeMap<String, String>,
) -> Result<CollectionDashboardAuthorityV1, String> {
    let source_by_key = collection
        .sources
        .iter()
        .map(|source| (source.key.as_str(), source))
        .collect::<BTreeMap<_, _>>();
    let mut membership = BTreeMap::<String, Vec<String>>::new();
    for set in &collection.runtime_sets {
        for member in &set.members {
            membership
                .entry(member.clone())
                .or_default()
                .push(set.id.clone());
        }
    }
    let clips = collection
        .clips
        .iter()
        .map(|clip| {
            let source = source_by_key
                .get(clip.source.as_str())
                .ok_or_else(|| "validated collection has missing source".to_owned())?;
            let facts = clip
                .check_key
                .as_ref()
                .and_then(|key| source.evidence.get(key));
            let coverage =
                facts.map_or_else(DashboardCoverageV1::default, |facts| DashboardCoverageV1 {
                    complete: facts.coverage.complete,
                    partial: facts.coverage.partial,
                    excluded: facts.coverage.excluded,
                    not_evaluated: facts.coverage.not_evaluated,
                });
            let outcome = if facts.is_some_and(|facts| facts.findings > 0) {
                "with_findings"
            } else if coverage.partial > 0 {
                "partial"
            } else if coverage.complete > 0 {
                "evaluated"
            } else if coverage.excluded > 0 {
                "excluded"
            } else if clip.check_key.is_none() {
                "unavailable"
            } else {
                "not_evaluated"
            };
            Ok(CollectionDashboardClipV1 {
                id: clip.id.clone(),
                source: clip.source.clone(),
                take_index: u64::from(clip.take_index),
                take_name: clip.take_name.clone(),
                roles: source.roles.clone(),
                availability: clip.availability.to_owned(),
                outcome: outcome.to_owned(),
                findings: facts.map_or(0, |facts| facts.findings),
                severities: facts
                    .map(|facts| facts.severities.iter().cloned().collect())
                    .unwrap_or_default(),
                coverage_gaps: facts.map_or(0, |facts| facts.coverage_gaps),
                prediction_unavailable: facts.map_or(0, |facts| facts.prediction_unavailable),
                coverage,
                runtime_sets: membership.remove(&clip.id).unwrap_or_default(),
                report_link: report_links.get(&clip.id).cloned(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(CollectionDashboardAuthorityV1 {
        schema: COLLECTION_DASHBOARD_V1_ID,
        schema_version: COLLECTION_DASHBOARD_V1_VERSION,
        collection_output,
        evaluation,
        summary: DashboardSummaryV1::from_rows(
            collection.sources.len(),
            &clips,
            &collection.runtime_sets,
        ),
        view: CollectionDashboardViewV1 {
            sources: collection
                .sources
                .iter()
                .map(|source| CollectionDashboardSourceV1 {
                    key: source.key.clone(),
                    locator: source.locator.clone(),
                    availability: source.availability.to_owned(),
                    loader: source.loader.to_owned(),
                    dependency_closure: source.dependency_closure.to_owned(),
                })
                .collect(),
            clips,
            runtime_sets: collection
                .runtime_sets
                .iter()
                .map(|set| CollectionDashboardRuntimeSetV1 {
                    id: set.id.clone(),
                    lifecycle: set.lifecycle.to_owned(),
                    members: set.members.clone(),
                    gaps: set.gaps.clone(),
                })
                .collect(),
        },
    })
}

#[derive(Serialize)]
struct CollectionDashboardAuthorityV1 {
    schema: &'static str,
    schema_version: u32,
    collection_output: InputIdentity,
    #[serde(skip_serializing_if = "Option::is_none")]
    evaluation: Option<EvaluationAuthorityV1>,
    summary: DashboardSummaryV1,
    view: CollectionDashboardViewV1,
}

#[derive(Serialize)]
struct DashboardSummaryV1 {
    sources: usize,
    clips: usize,
    runtime_sets: usize,
    findings: usize,
    coverage_gaps: usize,
    prediction_unavailable: usize,
    with_findings: usize,
    evaluated: usize,
    partial: usize,
    excluded: usize,
    unavailable: usize,
    not_evaluated: usize,
}
impl DashboardSummaryV1 {
    fn from_rows(
        sources: usize,
        clips: &[CollectionDashboardClipV1],
        sets: &[collection_output::CollectionDashboardRuntimeSetInput],
    ) -> Self {
        let mut value = Self {
            sources,
            clips: clips.len(),
            runtime_sets: sets.len(),
            findings: 0,
            coverage_gaps: 0,
            prediction_unavailable: 0,
            with_findings: 0,
            evaluated: 0,
            partial: 0,
            excluded: 0,
            unavailable: 0,
            not_evaluated: 0,
        };
        for clip in clips {
            value.findings += clip.findings;
            value.coverage_gaps += clip.coverage_gaps;
            value.prediction_unavailable += clip.prediction_unavailable;
            match clip.outcome.as_str() {
                "with_findings" => value.with_findings += 1,
                "evaluated" => value.evaluated += 1,
                "partial" => value.partial += 1,
                "excluded" => value.excluded += 1,
                "unavailable" => value.unavailable += 1,
                "not_evaluated" => value.not_evaluated += 1,
                _ => unreachable!("authority rows use closed outcome vocabulary"),
            }
        }
        value
    }
}

#[derive(Serialize)]
struct CollectionDashboardViewV1 {
    sources: Vec<CollectionDashboardSourceV1>,
    clips: Vec<CollectionDashboardClipV1>,
    runtime_sets: Vec<CollectionDashboardRuntimeSetV1>,
}

#[derive(Serialize)]
struct CollectionDashboardSourceV1 {
    key: String,
    locator: String,
    availability: String,
    loader: String,
    dependency_closure: String,
}

#[derive(Serialize)]
struct CollectionDashboardClipV1 {
    id: String,
    source: String,
    take_index: u64,
    take_name: String,
    roles: Vec<String>,
    availability: String,
    outcome: String,
    findings: usize,
    severities: Vec<String>,
    coverage_gaps: usize,
    prediction_unavailable: usize,
    coverage: DashboardCoverageV1,
    runtime_sets: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    report_link: Option<String>,
}

#[derive(Default, Serialize)]
struct DashboardCoverageV1 {
    complete: usize,
    partial: usize,
    excluded: usize,
    not_evaluated: usize,
}

#[derive(Serialize)]
struct CollectionDashboardRuntimeSetV1 {
    id: String,
    lifecycle: String,
    members: Vec<String>,
    gaps: Vec<String>,
}

#[allow(
    dead_code,
    reason = "strict dashboard authority readback validates these fields"
)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DashboardAuthorityReadback {
    schema: String,
    schema_version: u32,
    collection_output: IdentityWire,
    summary: DashboardSummaryReadback,
    #[serde(default)]
    evaluation: Option<DashboardEvaluationReadback>,
    view: DashboardViewReadback,
}
impl DashboardAuthorityReadback {
    fn validate_semantics(&self) -> Result<(), String> {
        if self.schema != COLLECTION_DASHBOARD_V1_ID
            || self.schema_version != COLLECTION_DASHBOARD_V1_VERSION
        {
            return Err("dashboard authority has the wrong V1 identity".to_owned());
        }
        let mut summary = DashboardSummaryReadback {
            sources: self.view.sources.len(),
            clips: self.view.clips.len(),
            runtime_sets: self.view.runtime_sets.len(),
            findings: 0,
            coverage_gaps: 0,
            prediction_unavailable: 0,
            with_findings: 0,
            evaluated: 0,
            partial: 0,
            excluded: 0,
            unavailable: 0,
            not_evaluated: 0,
        };
        let clip_ids = self
            .view
            .clips
            .iter()
            .map(|clip| clip.id.as_str())
            .collect::<BTreeSet<_>>();
        if clip_ids.len() != self.view.clips.len() {
            return Err("dashboard authority duplicates a logical clip".to_owned());
        }
        for clip in &self.view.clips {
            summary.findings = summary
                .findings
                .checked_add(clip.findings)
                .ok_or_else(|| "dashboard summary overflows".to_owned())?;
            summary.coverage_gaps = summary
                .coverage_gaps
                .checked_add(clip.coverage_gaps)
                .ok_or_else(|| "dashboard summary overflows".to_owned())?;
            summary.prediction_unavailable = summary
                .prediction_unavailable
                .checked_add(clip.prediction_unavailable)
                .ok_or_else(|| "dashboard summary overflows".to_owned())?;
            let expected = if clip.findings > 0 {
                "with_findings"
            } else if clip.coverage.partial > 0 {
                "partial"
            } else if clip.coverage.complete > 0 {
                "evaluated"
            } else if clip.coverage.excluded > 0 {
                "excluded"
            } else if clip.availability != "established" {
                "unavailable"
            } else {
                "not_evaluated"
            };
            if clip.outcome != expected {
                return Err("dashboard row outcome contradicts its coverage".to_owned());
            }
            match expected {
                "with_findings" => summary.with_findings += 1,
                "evaluated" => summary.evaluated += 1,
                "partial" => summary.partial += 1,
                "excluded" => summary.excluded += 1,
                "unavailable" => summary.unavailable += 1,
                "not_evaluated" => summary.not_evaluated += 1,
                _ => unreachable!(),
            }
        }
        for set in &self.view.runtime_sets {
            if set
                .members
                .iter()
                .any(|member| !clip_ids.contains(member.as_str()))
            {
                return Err("dashboard runtime set references an unknown logical clip".to_owned());
            }
        }
        if self.summary.sources != summary.sources
            || self.summary.clips != summary.clips
            || self.summary.runtime_sets != summary.runtime_sets
            || self.summary.findings != summary.findings
            || self.summary.coverage_gaps != summary.coverage_gaps
            || self.summary.prediction_unavailable != summary.prediction_unavailable
            || self.summary.with_findings != summary.with_findings
            || self.summary.evaluated != summary.evaluated
            || self.summary.partial != summary.partial
            || self.summary.excluded != summary.excluded
            || self.summary.unavailable != summary.unavailable
            || self.summary.not_evaluated != summary.not_evaluated
        {
            return Err("dashboard authority summary does not reconcile with rows".to_owned());
        }
        Ok(())
    }
}
#[allow(
    dead_code,
    reason = "strict dashboard authority readback validates these fields"
)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DashboardSummaryReadback {
    sources: usize,
    clips: usize,
    runtime_sets: usize,
    findings: usize,
    coverage_gaps: usize,
    prediction_unavailable: usize,
    with_findings: usize,
    evaluated: usize,
    partial: usize,
    excluded: usize,
    unavailable: usize,
    not_evaluated: usize,
}
#[allow(
    dead_code,
    reason = "strict dashboard authority readback validates these fields"
)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DashboardViewReadback {
    sources: Vec<DashboardSourceReadback>,
    clips: Vec<DashboardClipReadback>,
    runtime_sets: Vec<DashboardSetReadback>,
}
#[allow(
    dead_code,
    reason = "strict dashboard authority readback validates these fields"
)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DashboardSourceReadback {
    key: String,
    locator: String,
    availability: String,
    loader: String,
    dependency_closure: String,
}
#[allow(
    dead_code,
    reason = "strict dashboard authority readback validates these fields"
)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DashboardClipReadback {
    id: String,
    source: String,
    take_index: u64,
    take_name: String,
    roles: Vec<String>,
    availability: String,
    outcome: String,
    findings: usize,
    severities: Vec<String>,
    coverage_gaps: usize,
    prediction_unavailable: usize,
    coverage: DashboardCoverageReadback,
    runtime_sets: Vec<String>,
    #[serde(default)]
    report_link: Option<String>,
}
#[allow(
    dead_code,
    reason = "strict dashboard authority readback validates these fields"
)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DashboardCoverageReadback {
    complete: usize,
    partial: usize,
    excluded: usize,
    not_evaluated: usize,
}
#[allow(
    dead_code,
    reason = "strict dashboard authority readback validates these fields"
)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DashboardSetReadback {
    id: String,
    lifecycle: String,
    members: Vec<String>,
    gaps: Vec<String>,
}
#[allow(
    dead_code,
    reason = "strict dashboard authority readback validates these fields"
)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DashboardEvaluationReadback {
    input: IdentityWire,
    status: String,
    decision: String,
    #[serde(default)]
    reason: Option<String>,
    families: Vec<DashboardEvaluationFamilyReadback>,
}
#[allow(
    dead_code,
    reason = "strict dashboard authority readback validates these fields"
)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DashboardEvaluationFamilyReadback {
    id: String,
    status: String,
    decision: String,
    #[serde(default)]
    reason: Option<String>,
    members: Vec<DashboardEvaluationMemberReadback>,
    pair_findings: Vec<DashboardEvaluationPairFindingReadback>,
}

#[derive(Serialize)]
struct EvaluationPairFindingV1 {
    member_indices: [u64; 2],
    boundary: String,
    translation_offenders: Vec<TransitionPoseOffenderWire>,
    rotation_offenders: Vec<TransitionPoseOffenderWire>,
}
#[allow(
    dead_code,
    reason = "strict dashboard authority readback validates these fields"
)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DashboardEvaluationMemberReadback {
    take_index: u64,
    take_name: String,
    #[serde(default)]
    source_input: Option<IdentityWire>,
    #[serde(default)]
    logical_clip: Option<String>,
}

#[derive(Serialize)]
struct EvaluationAuthorityV1 {
    input: InputIdentity,
    status: String,
    decision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    families: Vec<EvaluationFamilyV1>,
}

#[derive(Serialize)]
struct EvaluationFamilyV1 {
    id: String,
    status: String,
    decision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    members: Vec<EvaluationMemberV1>,
    pair_findings: Vec<EvaluationPairFindingV1>,
}
#[allow(
    dead_code,
    reason = "strict dashboard authority readback validates these fields"
)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DashboardEvaluationPairFindingReadback {
    member_indices: [u64; 2],
    boundary: String,
    translation_offenders: Vec<TransitionPoseOffenderWire>,
    rotation_offenders: Vec<TransitionPoseOffenderWire>,
}

#[derive(Serialize)]
struct EvaluationMemberV1 {
    take_index: u64,
    take_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_input: Option<IdentityWire>,
    /// Logical member resolution is only recorded when source/take identity
    /// is unique in the collection; unmatched members remain explicit.
    #[serde(skip_serializing_if = "Option::is_none")]
    logical_clip: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IdentityWire {
    sha256: String,
    bytes: u64,
}

impl IdentityWire {
    fn from_input_identity(identity: &InputIdentity) -> Self {
        Self {
            sha256: identity.sha256().to_owned(),
            bytes: identity.bytes(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransitionPoseEvaluationWire {
    schema: String,
    schema_version: u32,
    status: String,
    decision: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(rename = "declaration_input")]
    _declaration_input: IdentityWire,
    #[serde(rename = "declaration_normalized")]
    _declaration_normalized: IdentityWire,
    subject_input: IdentityWire,
    #[serde(default, rename = "subject_dependency_closure_identity")]
    _subject_dependency_closure_identity: Option<IdentityWire>,
    families: Vec<TransitionPoseFamilyWire>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransitionPoseFamilyWire {
    family_id: String,
    status: String,
    decision: String,
    #[serde(default)]
    reason: Option<String>,
    members: Vec<TransitionPoseMemberWire>,
    #[serde(default, rename = "skeleton_basis_input")]
    _skeleton_basis_input: Option<IdentityWire>,
    pairs: Vec<TransitionPosePairWire>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransitionPoseMemberWire {
    take_index: u64,
    take_name: String,
    source_input: Option<IdentityWire>,
    #[serde(default, rename = "source_dependency_closure_identity")]
    _source_dependency_closure_identity: Option<IdentityWire>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TransitionPosePairWire {
    member_indices: [u64; 2],
    boundary: String,
    #[serde(rename = "max_translation_delta_m")]
    _max_translation_delta_m: f64,
    #[serde(rename = "max_rotation_delta_deg")]
    _max_rotation_delta_deg: f64,
    #[serde(rename = "translation_tolerance_m")]
    _translation_tolerance_m: f64,
    #[serde(rename = "rotation_tolerance_deg")]
    _rotation_tolerance_deg: f64,
    translation_offenders: Vec<TransitionPoseOffenderWire>,
    rotation_offenders: Vec<TransitionPoseOffenderWire>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransitionPoseOffenderWire {
    #[serde(rename = "bone_ordinal")]
    bone_ordinal: u64,
    #[serde(rename = "bone_name")]
    bone_name: String,
    #[serde(rename = "delta", alias = "delta_m", alias = "delta_deg")]
    delta: f64,
}

#[cfg(test)]
mod tests {
    use super::{
        BoundedWriter, read_bounded, safe_relative_report_reference, validate_authority_readback,
    };
    use std::io::Write;

    #[test]
    fn safe_report_references_reject_paths_and_web_syntax() {
        assert!(safe_relative_report_reference("reports/walk.html"));
        for value in [
            "/report.html",
            "../report.html",
            "a/../report.html",
            "https://x",
            "report.html#x",
            "report.html?q=1",
            "a\\b.html",
            "reports/%2e%2e/escape.html",
        ] {
            assert!(!safe_relative_report_reference(value), "{value}");
        }
        assert!(!safe_relative_report_reference(&"x".repeat(4097)));
    }

    #[test]
    fn readers_and_authority_counter_fail_closed_at_their_limits() {
        let temporary = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(temporary.path(), [1_u8, 2, 3]).unwrap();
        assert!(read_bounded(temporary.path(), 2, "test").is_err());
        let mut writer = BoundedWriter::new(2);
        assert!(writer.write_all(&[1_u8, 2]).is_ok());
        assert!(writer.write_all(&[3]).is_err());
    }

    #[test]
    fn authority_readback_rejects_schema_and_summary_mutations() {
        let identity = "0".repeat(64);
        let value = serde_json::json!({
            "schema": "urn:animsmith:schema:collection-dashboard:1", "schema_version": 1,
            "collection_output": {"sha256": identity, "bytes": 0},
            "summary": {"sources": 0, "clips": 0, "runtime_sets": 0, "findings": 0, "coverage_gaps": 0, "prediction_unavailable": 0, "with_findings": 0, "evaluated": 0, "partial": 0, "excluded": 0, "unavailable": 0, "not_evaluated": 0},
            "view": {"sources": [], "clips": [], "runtime_sets": []}
        });
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(validate_authority_readback(&bytes).is_ok());
        let mut mismatch = value.clone();
        mismatch["summary"]["clips"] = 1.into();
        assert!(validate_authority_readback(&serde_json::to_vec(&mismatch).unwrap()).is_err());
        let mut unknown = value;
        unknown["unexpected"] = true.into();
        assert!(validate_authority_readback(&serde_json::to_vec(&unknown).unwrap()).is_err());
    }
}
