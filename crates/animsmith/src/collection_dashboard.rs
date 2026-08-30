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
    let findings = authority_value.summary.findings;
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
        findings,
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
    let readback = serde_json::from_slice::<CollectionDashboardAuthorityV1>(bytes)
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
    let wire: TransitionPoseEvaluationWire = serde_json::from_slice(&bytes)
        .map_err(|_| "transition-pose evaluation has an unsupported V1 shape".to_owned())?;
    if wire.schema != TRANSITION_POSE_V1_ID
        || wire.schema_version != 1
        || wire.subject_input != IdentityWire::from_input_identity(&collection.manifest)
    {
        return Err(
            "transition-pose evaluation is not compatible with this collection output".to_owned(),
        );
    }
    wire.validate()?;
    let source_inputs = collection
        .sources
        .iter()
        .filter_map(|source| {
            source.input.as_ref().map(|input| {
                (
                    source.key.as_str(),
                    IdentityWire::from_input_identity(input),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    Ok(EvaluationAuthorityV1 {
        input: IdentityWire::from_input_identity(&InputIdentity::from_bytes(&bytes)),
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
                        let logical_clip = resolve_logical_clip(
                            member.source_input.as_ref(),
                            member.take_index,
                            &member.take_name,
                            &source_inputs,
                            collection.clips.iter().map(|clip| {
                                (
                                    clip.id.as_str(),
                                    clip.source.as_str(),
                                    u64::from(clip.take_index),
                                    clip.take_name.as_str(),
                                )
                            }),
                        );
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
                            translation_offenders: pair
                                .translation_offenders
                                .into_iter()
                                .map(TransitionPoseOffenderWire::translation)
                                .collect(),
                            rotation_offenders: pair
                                .rotation_offenders
                                .into_iter()
                                .map(TransitionPoseOffenderWire::rotation)
                                .collect(),
                        })
                    })
                    .collect(),
            })
            .collect(),
    })
}

/// Resolve a transition member only when its complete physical witness names
/// exactly one logical declaration. The same helper is used by projection and
/// strict authority readback so serialized `logical_clip` is never trusted.
fn resolve_logical_clip<'a>(
    source_input: Option<&IdentityWire>,
    take_index: u64,
    take_name: &str,
    source_inputs: &BTreeMap<&str, IdentityWire>,
    clips: impl IntoIterator<Item = (&'a str, &'a str, u64, &'a str)>,
) -> Option<String> {
    let source_input = source_input?;
    let mut resolved = None;
    for (id, source, candidate_index, candidate_name) in clips {
        if candidate_index == take_index
            && candidate_name == take_name
            && source_inputs.get(source) == Some(source_input)
        {
            if resolved.is_some() {
                return None;
            }
            resolved = Some(id.to_owned());
        }
    }
    resolved
}

/// An established logical row is only meaningful when its declared source,
/// source-take index, and take name identify exactly one established physical
/// row. Projection and strict readback share this lookup.
fn reconciled_established_physical_take<'a>(
    source_key: &str,
    take_index: u64,
    take_name: &str,
    availability: &str,
    sources: &'a [CollectionDashboardSourceV1],
) -> Result<Option<&'a CollectionDashboardPhysicalTakeV1>, String> {
    if availability != "established" {
        return Ok(None);
    }
    let mut source_matches = sources.iter().filter(|source| source.key == source_key);
    let source = source_matches
        .next()
        .ok_or_else(|| "established dashboard clip has no physical source".to_owned())?;
    if source_matches.next().is_some() {
        return Err("established dashboard clip has an ambiguous physical source".to_owned());
    }
    let mut take_matches = source.takes.iter().filter(|take| {
        take.source_take_index == take_index && take.take_name.as_deref() == Some(take_name)
    });
    let take = take_matches
        .next()
        .ok_or_else(|| "established dashboard clip has no exact physical take".to_owned())?;
    if take_matches.next().is_some() || take.availability != "established" {
        return Err("established dashboard clip has an ambiguous physical take".to_owned());
    }
    Ok(Some(take))
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
    let sources = collection
        .sources
        .iter()
        .map(|source| {
            let mut normalized_name_counts = BTreeMap::<&str, usize>::new();
            for name in source
                .takes
                .iter()
                .filter_map(|take| take.normalized_clip.as_ref().map(|(_, name)| name.as_str()))
            {
                *normalized_name_counts.entry(name).or_default() += 1;
            }
            CollectionDashboardSourceV1 {
                key: source.key.clone(),
                locator: source.locator.clone(),
                input: source.input.as_ref().map(IdentityWire::from_input_identity),
                availability: source.availability.to_owned(),
                loader: source.loader.to_owned(),
                dependency_closure: source.dependency_closure.to_owned(),
                takes: source
                    .takes
                    .iter()
                    .map(|take| {
                        let facts = take
                            .normalized_clip
                            .as_ref()
                            .and_then(|(_, name)| source.evidence.get(name));
                        let availability = match (&take.take_name, &take.normalized_clip) {
                            (_, Some((_, name)))
                                if normalized_name_counts.get(name.as_str()) != Some(&1) =>
                            {
                                "duplicate_normalized_clip_name"
                            }
                            (Some(_), Some(_)) => "established",
                            (None, _) => "take_name_unavailable",
                            (Some(_), None) => "normalized_clip_unavailable",
                        };
                        let evidence = project_evidence(facts, availability);
                        CollectionDashboardPhysicalTakeV1 {
                            source_take_index: u64::from(take.source_take_index),
                            take_name: take.take_name.clone(),
                            normalized_clip_index: take
                                .normalized_clip
                                .as_ref()
                                .map(|(index, _)| u64::from(*index)),
                            normalized_clip_name: take
                                .normalized_clip
                                .as_ref()
                                .map(|(_, name)| name.clone()),
                            availability: availability.to_owned(),
                            outcome: evidence.outcome,
                            findings: evidence.findings,
                            severities: evidence.severities,
                            coverage_gaps: evidence.coverage_gaps,
                            prediction_unavailable: evidence.prediction_unavailable,
                            coverage: evidence.coverage,
                        }
                    })
                    .collect(),
                unscoped_findings: source.unscoped_findings,
                unscoped_severities: source.unscoped_severities.iter().cloned().collect(),
            }
        })
        .collect::<Vec<_>>();
    let clips = collection
        .clips
        .iter()
        .map(|clip| {
            let source = source_by_key
                .get(clip.source.as_str())
                .ok_or_else(|| "validated collection has missing source".to_owned())?;
            let evidence = match reconciled_established_physical_take(
                &clip.source,
                u64::from(clip.take_index),
                &clip.take_name,
                clip.availability,
                &sources,
            )? {
                Some(take) => project_physical_evidence(take),
                None => project_evidence(None, clip.availability),
            };
            Ok(CollectionDashboardClipV1 {
                id: clip.id.clone(),
                source: clip.source.clone(),
                take_index: u64::from(clip.take_index),
                take_name: clip.take_name.clone(),
                roles: source.roles.clone(),
                availability: clip.availability.to_owned(),
                outcome: evidence.outcome,
                findings: evidence.findings,
                severities: evidence.severities,
                coverage_gaps: evidence.coverage_gaps,
                prediction_unavailable: evidence.prediction_unavailable,
                coverage: evidence.coverage,
                runtime_sets: membership.remove(&clip.id).unwrap_or_default(),
                report_link: report_links.get(&clip.id).cloned(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(CollectionDashboardAuthorityV1 {
        schema: COLLECTION_DASHBOARD_V1_ID.to_owned(),
        schema_version: COLLECTION_DASHBOARD_V1_VERSION,
        collection_output: IdentityWire::from_input_identity(&collection_output),
        evaluation,
        summary: DashboardSummaryV1::from_rows(&sources, &clips, &collection.runtime_sets),
        view: CollectionDashboardViewV1 {
            sources,
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

struct DashboardEvidenceProjection {
    outcome: String,
    findings: usize,
    severities: Vec<String>,
    coverage_gaps: usize,
    prediction_unavailable: usize,
    coverage: DashboardCoverageV1,
}

fn project_physical_evidence(
    take: &CollectionDashboardPhysicalTakeV1,
) -> DashboardEvidenceProjection {
    DashboardEvidenceProjection {
        outcome: take.outcome.clone(),
        findings: take.findings,
        severities: take.severities.clone(),
        coverage_gaps: take.coverage_gaps,
        prediction_unavailable: take.prediction_unavailable,
        coverage: take.coverage.clone(),
    }
}

fn project_evidence(
    facts: Option<&collection_output::CollectionDashboardClipEvidence>,
    availability: &str,
) -> DashboardEvidenceProjection {
    let coverage = facts.map_or_else(DashboardCoverageV1::default, |facts| DashboardCoverageV1 {
        complete: facts.coverage.complete,
        partial: facts.coverage.partial,
        excluded: facts.coverage.excluded,
        not_evaluated: facts.coverage.not_evaluated,
    });
    DashboardEvidenceProjection {
        outcome: derive_evidence_outcome(
            facts.map_or(0, |facts| facts.findings),
            &coverage,
            availability,
        )
        .to_owned(),
        findings: facts.map_or(0, |facts| facts.findings),
        severities: facts
            .map(|facts| facts.severities.iter().cloned().collect())
            .unwrap_or_default(),
        coverage_gaps: facts.map_or(0, |facts| facts.coverage_gaps),
        prediction_unavailable: facts.map_or(0, |facts| facts.prediction_unavailable),
        coverage,
    }
}

fn derive_evidence_outcome(
    findings: usize,
    coverage: &DashboardCoverageV1,
    availability: &str,
) -> &'static str {
    if findings > 0 {
        "with_findings"
    } else if coverage.partial > 0 {
        "partial"
    } else if coverage.complete > 0 {
        "evaluated"
    } else if coverage.excluded > 0 {
        "excluded"
    } else if availability != "established" {
        "unavailable"
    } else {
        "not_evaluated"
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionDashboardAuthorityV1 {
    schema: String,
    schema_version: u32,
    collection_output: IdentityWire,
    #[serde(skip_serializing_if = "Option::is_none")]
    evaluation: Option<EvaluationAuthorityV1>,
    summary: DashboardSummaryV1,
    view: CollectionDashboardViewV1,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DashboardSummaryV1 {
    sources: usize,
    physical_takes: usize,
    clips: usize,
    runtime_sets: usize,
    findings: usize,
    unscoped_findings: usize,
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
        sources: &[CollectionDashboardSourceV1],
        clips: &[CollectionDashboardClipV1],
        sets: &[collection_output::CollectionDashboardRuntimeSetInput],
    ) -> Self {
        let unscoped_findings = sources
            .iter()
            .map(|source| source.unscoped_findings)
            .sum::<usize>();
        let mut value = Self {
            sources: sources.len(),
            physical_takes: sources.iter().map(|source| source.takes.len()).sum(),
            clips: clips.len(),
            runtime_sets: sets.len(),
            findings: unscoped_findings,
            unscoped_findings,
            coverage_gaps: 0,
            prediction_unavailable: 0,
            with_findings: 0,
            evaluated: 0,
            partial: 0,
            excluded: 0,
            unavailable: 0,
            not_evaluated: 0,
        };
        for take in sources.iter().flat_map(|source| &source.takes) {
            value.findings += take.findings;
            value.coverage_gaps += take.coverage_gaps;
            value.prediction_unavailable += take.prediction_unavailable;
        }
        for clip in clips {
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

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionDashboardViewV1 {
    sources: Vec<CollectionDashboardSourceV1>,
    clips: Vec<CollectionDashboardClipV1>,
    runtime_sets: Vec<CollectionDashboardRuntimeSetV1>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionDashboardSourceV1 {
    key: String,
    locator: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<IdentityWire>,
    availability: String,
    loader: String,
    dependency_closure: String,
    takes: Vec<CollectionDashboardPhysicalTakeV1>,
    unscoped_findings: usize,
    unscoped_severities: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionDashboardPhysicalTakeV1 {
    source_take_index: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    take_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    normalized_clip_index: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    normalized_clip_name: Option<String>,
    availability: String,
    outcome: String,
    findings: usize,
    severities: Vec<String>,
    coverage_gaps: usize,
    prediction_unavailable: usize,
    coverage: DashboardCoverageV1,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DashboardCoverageV1 {
    complete: usize,
    partial: usize,
    excluded: usize,
    not_evaluated: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionDashboardRuntimeSetV1 {
    id: String,
    lifecycle: String,
    members: Vec<String>,
    gaps: Vec<String>,
}

impl CollectionDashboardAuthorityV1 {
    fn validate_semantics(&self) -> Result<(), String> {
        if self.schema != COLLECTION_DASHBOARD_V1_ID
            || self.schema_version != COLLECTION_DASHBOARD_V1_VERSION
            || !self.collection_output.valid(None)
            || self.view.sources.len() > 4096
            || self.view.clips.len() > 4096
            || self.view.runtime_sets.len() > 4096
        {
            return Err("dashboard authority has the wrong V1 identity".to_owned());
        }
        if let Some(evaluation) = &self.evaluation {
            evaluation.validate()?;
        }
        let mut summary = DashboardSummaryV1 {
            sources: self.view.sources.len(),
            physical_takes: 0,
            clips: self.view.clips.len(),
            runtime_sets: self.view.runtime_sets.len(),
            findings: 0,
            unscoped_findings: 0,
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
            if clip.id.chars().count() > 4096
                || clip.source.chars().count() > 4096
                || clip.take_name.chars().count() > 4096
                || !matches!(
                    clip.availability.as_str(),
                    "established"
                        | "duplicate_embedded_take_name"
                        | "nested_output_unavailable"
                        | "source_unavailable"
                        | "digest_mismatched"
                        | "loader_unavailable"
                        | "dependency_closure_incomplete"
                        | "document_unavailable"
                        | "take_inventory_unavailable"
                        | "take_index_missing"
                        | "take_name_unavailable"
                        | "take_name_mismatched"
                        | "normalized_clip_unavailable"
                )
                || clip.roles.len() > 4096
                || clip.severities.len() > 4096
                || clip.runtime_sets.len() > 4096
                || clip.roles.iter().any(|role| role.chars().count() > 4096)
                || clip
                    .runtime_sets
                    .iter()
                    .any(|set| set.chars().count() > 4096)
                || clip
                    .severities
                    .iter()
                    .any(|severity| !matches!(severity.as_str(), "error" | "warning" | "note"))
                || clip
                    .report_link
                    .as_ref()
                    .is_some_and(|link| !safe_relative_report_reference(link))
            {
                return Err("dashboard authority has an invalid bounded row".to_owned());
            }
            let expected =
                derive_evidence_outcome(clip.findings, &clip.coverage, &clip.availability);
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
        let source_keys = self
            .view
            .sources
            .iter()
            .map(|source| source.key.as_str())
            .collect::<BTreeSet<_>>();
        if source_keys.len() != self.view.sources.len() {
            return Err("dashboard authority duplicates a source key".to_owned());
        }
        for source in &self.view.sources {
            if source.key.chars().count() > 4096
                || source.locator.chars().count() > 4096
                || source
                    .input
                    .as_ref()
                    .is_some_and(|input| !input.valid(None))
                || (source.availability == "available") != source.input.is_some()
                || !matches!(source.availability.as_str(), "available" | "unavailable")
                || !matches!(source.loader.as_str(), "ready" | "unavailable")
                || !matches!(
                    source.dependency_closure.as_str(),
                    "complete" | "partial" | "unavailable"
                )
                || source.takes.len() > 4096
                || source.availability == "unavailable" && !source.takes.is_empty()
                || source.unscoped_severities.len() > 4096
                || source
                    .unscoped_severities
                    .iter()
                    .any(|severity| !matches!(severity.as_str(), "error" | "warning" | "note"))
                || source
                    .unscoped_severities
                    .iter()
                    .collect::<BTreeSet<_>>()
                    .len()
                    != source.unscoped_severities.len()
                || (source.unscoped_findings == 0) != source.unscoped_severities.is_empty()
            {
                return Err("dashboard authority has an invalid source".to_owned());
            }
            let mut source_take_indices = BTreeSet::new();
            let mut normalized_clip_indices = BTreeSet::new();
            let mut normalized_name_counts = BTreeMap::<&str, usize>::new();
            for name in source
                .takes
                .iter()
                .filter_map(|take| take.normalized_clip_name.as_deref())
            {
                *normalized_name_counts.entry(name).or_default() += 1;
            }
            for take in &source.takes {
                let normalized_identity_present =
                    take.normalized_clip_index.is_some() && take.normalized_clip_name.is_some();
                if !source_take_indices.insert(take.source_take_index)
                    || take
                        .take_name
                        .as_ref()
                        .is_some_and(|name| name.is_empty() || name.chars().count() > 4096)
                    || take
                        .normalized_clip_name
                        .as_ref()
                        .is_some_and(|name| name.is_empty() || name.chars().count() > 4101)
                    || take.normalized_clip_index.is_some() != take.normalized_clip_name.is_some()
                    || take.normalized_clip_name.as_ref().is_some_and(|name| {
                        let duplicate = normalized_name_counts
                            .get(name.as_str())
                            .is_some_and(|count| *count > 1);
                        (take.availability == "duplicate_normalized_clip_name") != duplicate
                    })
                    || take
                        .normalized_clip_index
                        .is_some_and(|index| !normalized_clip_indices.insert(index))
                    || !matches!(
                        (
                            take.availability.as_str(),
                            take.take_name.is_some(),
                            normalized_identity_present
                        ),
                        ("established", true, true)
                            | ("duplicate_normalized_clip_name", true, true)
                            | ("duplicate_normalized_clip_name", false, true)
                            | ("take_name_unavailable", false, false)
                            | ("take_name_unavailable", false, true)
                            | ("normalized_clip_unavailable", true, false)
                    )
                    || take.severities.len() > 4096
                    || take
                        .severities
                        .iter()
                        .any(|severity| !matches!(severity.as_str(), "error" | "warning" | "note"))
                    || take.severities.iter().collect::<BTreeSet<_>>().len()
                        != take.severities.len()
                    || (take.findings == 0) != take.severities.is_empty()
                {
                    return Err("dashboard authority has an invalid physical take".to_owned());
                }
                let expected =
                    derive_evidence_outcome(take.findings, &take.coverage, &take.availability);
                if take.outcome != expected {
                    return Err(
                        "dashboard physical-take outcome contradicts its evidence".to_owned()
                    );
                }
                summary.physical_takes = summary
                    .physical_takes
                    .checked_add(1)
                    .ok_or_else(|| "dashboard summary overflows".to_owned())?;
                summary.findings = summary
                    .findings
                    .checked_add(take.findings)
                    .ok_or_else(|| "dashboard summary overflows".to_owned())?;
                summary.coverage_gaps = summary
                    .coverage_gaps
                    .checked_add(take.coverage_gaps)
                    .ok_or_else(|| "dashboard summary overflows".to_owned())?;
                summary.prediction_unavailable = summary
                    .prediction_unavailable
                    .checked_add(take.prediction_unavailable)
                    .ok_or_else(|| "dashboard summary overflows".to_owned())?;
            }
            summary.findings = summary
                .findings
                .checked_add(source.unscoped_findings)
                .ok_or_else(|| "dashboard summary overflows".to_owned())?;
            summary.unscoped_findings = summary
                .unscoped_findings
                .checked_add(source.unscoped_findings)
                .ok_or_else(|| "dashboard summary overflows".to_owned())?;
        }
        if self
            .view
            .clips
            .iter()
            .any(|clip| !source_keys.contains(clip.source.as_str()))
        {
            return Err("dashboard clip references an unknown source".to_owned());
        }
        for clip in &self.view.clips {
            if let Some(take) = reconciled_established_physical_take(
                &clip.source,
                clip.take_index,
                &clip.take_name,
                &clip.availability,
                &self.view.sources,
            )? && (clip.outcome != take.outcome
                || clip.findings != take.findings
                || clip.severities != take.severities
                || clip.coverage_gaps != take.coverage_gaps
                || clip.prediction_unavailable != take.prediction_unavailable
                || clip.coverage != take.coverage)
            {
                return Err(
                    "established dashboard clip evidence does not reconcile with its physical take"
                        .to_owned(),
                );
            }
        }
        let source_inputs = self
            .view
            .sources
            .iter()
            .filter_map(|source| {
                source
                    .input
                    .as_ref()
                    .map(|input| (source.key.as_str(), input.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        if self.evaluation.as_ref().is_some_and(|evaluation| {
            evaluation.families.iter().any(|family| {
                family.members.iter().any(|member| {
                    let expected = resolve_logical_clip(
                        member.source_input.as_ref(),
                        member.take_index,
                        &member.take_name,
                        &source_inputs,
                        self.view.clips.iter().map(|clip| {
                            (
                                clip.id.as_str(),
                                clip.source.as_str(),
                                clip.take_index,
                                clip.take_name.as_str(),
                            )
                        }),
                    );
                    member.logical_clip != expected
                })
            })
        }) {
            return Err(
                "dashboard evaluation logical resolution contradicts its physical witness"
                    .to_owned(),
            );
        }
        let mut memberships = BTreeMap::<&str, BTreeSet<&str>>::new();
        let mut set_ids = BTreeSet::new();
        for set in &self.view.runtime_sets {
            if set.id.chars().count() > 4096
                || set.members.len() > 4096
                || set.gaps.len() > 4096
                || set
                    .members
                    .iter()
                    .any(|member| member.chars().count() > 4096)
                || set.gaps.iter().any(|gap| gap.chars().count() > 4096)
                || !matches!(set.lifecycle.as_str(), "complete" | "incomplete")
            {
                return Err("dashboard authority has an invalid runtime set".to_owned());
            }
            if !set_ids.insert(set.id.as_str()) {
                return Err("dashboard authority duplicates a runtime set".to_owned());
            }
            if set
                .members
                .iter()
                .any(|member| !clip_ids.contains(member.as_str()))
            {
                return Err("dashboard runtime set references an unknown logical clip".to_owned());
            }
            for member in &set.members {
                if !memberships
                    .entry(member.as_str())
                    .or_default()
                    .insert(set.id.as_str())
                {
                    return Err("dashboard runtime set duplicates a logical clip".to_owned());
                }
            }
        }
        for clip in &self.view.clips {
            let listed = clip
                .runtime_sets
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            if listed.len() != clip.runtime_sets.len()
                || memberships
                    .get(clip.id.as_str())
                    .is_some_and(|sets| sets != &listed)
                || !memberships.contains_key(clip.id.as_str()) && !listed.is_empty()
            {
                return Err("dashboard clip/runtime-set membership does not reconcile".to_owned());
            }
        }
        if self.summary.sources != summary.sources
            || self.summary.physical_takes != summary.physical_takes
            || self.summary.clips != summary.clips
            || self.summary.runtime_sets != summary.runtime_sets
            || self.summary.findings != summary.findings
            || self.summary.unscoped_findings != summary.unscoped_findings
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
impl EvaluationAuthorityV1 {
    fn validate(&self) -> Result<(), String> {
        if !self.input.valid(Some(MAX_EVALUATION_BYTES))
            || !matches!(self.status.as_str(), "complete" | "incomplete")
            || !matches!(self.decision.as_str(), "pass" | "finding" | "not_evaluated")
            || self.reason.as_deref().is_some_and(|reason| {
                !valid_transition_reason(reason) && reason != "no_configured_families"
            })
            || self.families.len() > 4096
        {
            return Err("dashboard authority has invalid transition evaluation".to_owned());
        }
        for family in &self.families {
            family.validate()?;
        }
        let complete_pass = self.status == "complete" && self.decision == "pass";
        let complete_finding = self.status == "complete" && self.decision == "finding";
        let incomplete = self.status == "incomplete" && self.decision == "not_evaluated";
        let valid = (complete_pass
            && ((self.reason.as_deref() == Some("no_configured_families")
                && self.families.is_empty())
                || (self.reason.is_none()
                    && !self.families.is_empty()
                    && self
                        .families
                        .iter()
                        .all(|family| family.status == "complete" && family.decision == "pass"))))
            || (complete_finding
                && self.reason.is_none()
                && !self.families.is_empty()
                && self
                    .families
                    .iter()
                    .all(|family| family.status == "complete")
                && self
                    .families
                    .iter()
                    .any(|family| family.decision == "finding"))
            || (incomplete
                && self.reason.is_none()
                && !self.families.is_empty()
                && self
                    .families
                    .iter()
                    .any(|family| family.status == "incomplete"));
        valid
            .then_some(())
            .ok_or_else(|| "dashboard authority has contradictory transition state".to_owned())
    }
}
impl EvaluationFamilyV1 {
    fn validate(&self) -> Result<(), String> {
        if self.id.is_empty()
            || self.id.chars().count() > 255
            || !matches!(self.status.as_str(), "complete" | "incomplete")
            || !matches!(self.decision.as_str(), "pass" | "finding" | "not_evaluated")
            || self
                .reason
                .as_deref()
                .is_some_and(|reason| !valid_transition_reason(reason))
            || self.members.len() > 4096
            || self.pair_findings.len() > 4096
        {
            return Err("dashboard authority has invalid transition family".to_owned());
        }
        for member in &self.members {
            if member.take_name.is_empty()
                || member.take_name.chars().count() > 4096
                || member
                    .source_input
                    .as_ref()
                    .is_some_and(|input| !input.valid(None))
                || member
                    .logical_clip
                    .as_ref()
                    .is_some_and(|clip| clip.chars().count() > 4096)
            {
                return Err("dashboard authority has invalid transition member".to_owned());
            }
        }
        for pair in &self.pair_findings {
            pair.validate(self.members.len())?;
        }
        let complete_pass = self.status == "complete" && self.decision == "pass";
        let complete_finding = self.status == "complete" && self.decision == "finding";
        let incomplete = self.status == "incomplete" && self.decision == "not_evaluated";
        let valid = (complete_pass && self.reason.is_none() && self.pair_findings.is_empty())
            || (complete_finding && self.reason.is_none() && !self.pair_findings.is_empty())
            || (incomplete
                && self.reason.as_deref().is_some_and(valid_transition_reason)
                && self.pair_findings.is_empty());
        valid
            .then_some(())
            .ok_or_else(|| "dashboard authority has contradictory transition family".to_owned())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvaluationPairFindingV1 {
    member_indices: [u64; 2],
    boundary: String,
    translation_offenders: Vec<TransitionPoseOffenderWire>,
    rotation_offenders: Vec<TransitionPoseOffenderWire>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvaluationAuthorityV1 {
    input: IdentityWire,
    status: String,
    decision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    families: Vec<EvaluationFamilyV1>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvaluationFamilyV1 {
    id: String,
    status: String,
    decision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    members: Vec<EvaluationMemberV1>,
    pair_findings: Vec<EvaluationPairFindingV1>,
}
impl EvaluationPairFindingV1 {
    fn validate(&self, members: usize) -> Result<(), String> {
        if self
            .member_indices
            .iter()
            .any(|index| *index >= members as u64)
            || !matches!(self.boundary.as_str(), "entry" | "exit")
            || self.translation_offenders.len() > 16
            || self.rotation_offenders.len() > 16
            || self.translation_offenders.is_empty() && self.rotation_offenders.is_empty()
        {
            return Err("dashboard authority has invalid transition pair finding".to_owned());
        }
        for offender in &self.translation_offenders {
            offender.validate()?;
        }
        for offender in &self.rotation_offenders {
            offender.validate()?;
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    fn valid(&self, max_bytes: Option<u64>) -> bool {
        self.sha256.len() == 64
            && self
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (byte.is_ascii_lowercase() && byte <= b'f'))
            && max_bytes.is_none_or(|maximum| self.bytes <= maximum)
    }
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

impl TransitionPoseEvaluationWire {
    fn validate(&self) -> Result<(), String> {
        if self.schema != TRANSITION_POSE_V1_ID
            || self.schema_version != 1
            || !self._declaration_input.valid(Some(MAX_EVALUATION_BYTES))
            || !self
                ._declaration_normalized
                .valid(Some(MAX_EVALUATION_BYTES))
            || !self.subject_input.valid(None)
            || self
                ._subject_dependency_closure_identity
                .as_ref()
                .is_some_and(|value| !value.valid(None))
            || self.families.len() > 4096
        {
            return Err("transition-pose evaluation has an unsupported V1 shape".to_owned());
        }
        for family in &self.families {
            family.validate()?;
        }
        let complete_pass = self.status == "complete" && self.decision == "pass";
        let complete_finding = self.status == "complete" && self.decision == "finding";
        let incomplete = self.status == "incomplete" && self.decision == "not_evaluated";
        let valid = (complete_pass
            && ((self.reason.as_deref() == Some("no_configured_families")
                && self.families.is_empty())
                || (self.reason.is_none()
                    && !self.families.is_empty()
                    && self
                        .families
                        .iter()
                        .all(|family| family.status == "complete" && family.decision == "pass"))))
            || (complete_finding
                && self.reason.is_none()
                && !self.families.is_empty()
                && self
                    .families
                    .iter()
                    .all(|family| family.status == "complete")
                && self
                    .families
                    .iter()
                    .any(|family| family.decision == "finding"))
            || (incomplete
                && self.reason.is_none()
                && !self.families.is_empty()
                && self
                    .families
                    .iter()
                    .any(|family| family.status == "incomplete"));
        valid
            .then_some(())
            .ok_or_else(|| "transition-pose evaluation has contradictory V1 state".to_owned())
    }
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

impl TransitionPoseFamilyWire {
    fn validate(&self) -> Result<(), String> {
        if self.family_id.is_empty()
            || self.family_id.chars().count() > 255
            || self.members.len() < 2
            || self.members.len() > 4096
            || self.pairs.len() > 4096
        {
            return Err("transition-pose evaluation has an unsupported V1 shape".to_owned());
        }
        for member in &self.members {
            member.validate()?;
        }
        for pair in &self.pairs {
            pair.validate(self.members.len())?;
        }
        let complete_pass = self.status == "complete" && self.decision == "pass";
        let complete_finding = self.status == "complete" && self.decision == "finding";
        let incomplete = self.status == "incomplete" && self.decision == "not_evaluated";
        let all_available = self.members.iter().all(|member| {
            member.source_input.is_some() && member._source_dependency_closure_identity.is_some()
        });
        let incomplete_members_valid = match self.reason.as_deref() {
            Some("dependency_closure_incomplete") => self
                .members
                .iter()
                .any(|member| member._source_dependency_closure_identity.is_none()),
            Some("member_unavailable") => true,
            Some(_) => all_available,
            None => false,
        };
        let valid = (complete_pass
            && self.reason.is_none()
            && all_available
            && self
                ._skeleton_basis_input
                .as_ref()
                .is_some_and(|identity| identity.valid(None))
            && self.has_canonical_complete_pair_coverage()
            && self.pairs.iter().all(|pair| {
                pair.translation_offenders.is_empty() && pair.rotation_offenders.is_empty()
            }))
            || (complete_finding
                && self.reason.is_none()
                && all_available
                && self
                    ._skeleton_basis_input
                    .as_ref()
                    .is_some_and(|identity| identity.valid(None))
                && self.has_canonical_complete_pair_coverage()
                && self.pairs.iter().any(|pair| {
                    !pair.translation_offenders.is_empty() || !pair.rotation_offenders.is_empty()
                }))
            || (incomplete
                && self.reason.as_deref().is_some_and(valid_transition_reason)
                && self.pairs.is_empty()
                && incomplete_members_valid);
        valid
            .then_some(())
            .ok_or_else(|| "transition-pose evaluation has contradictory family state".to_owned())
    }

    /// The V1 producer compares every unordered member pair in ascending
    /// member order, then emits entry before exit when both boundaries were
    /// declared. The result does not repeat the declaration boundary, so the
    /// three canonical producer shapes are entry-only, exit-only, or both.
    fn has_canonical_complete_pair_coverage(&self) -> bool {
        let Some(pair_count) = self
            .members
            .len()
            .checked_mul(self.members.len().saturating_sub(1))
            .map(|value| value / 2)
        else {
            return false;
        };
        let boundaries: &[&str] = match (
            self.pairs.len(),
            self.pairs.first().map(|pair| pair.boundary.as_str()),
        ) {
            (length, Some("entry")) if length == pair_count => &["entry"],
            (length, Some("exit")) if length == pair_count => &["exit"],
            (length, Some("entry")) if pair_count.checked_mul(2) == Some(length) => {
                &["entry", "exit"]
            }
            _ => return false,
        };
        let mut actual = self.pairs.iter();
        for left in 0..self.members.len() {
            for right in left + 1..self.members.len() {
                for boundary in boundaries {
                    let Some(pair) = actual.next() else {
                        return false;
                    };
                    if pair.member_indices != [left as u64, right as u64]
                        || pair.boundary != *boundary
                    {
                        return false;
                    }
                }
            }
        }
        actual.next().is_none()
    }
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

impl TransitionPoseMemberWire {
    fn validate(&self) -> Result<(), String> {
        if self.take_name.is_empty()
            || self.take_name.chars().count() > 4096
            || self
                .source_input
                .as_ref()
                .is_some_and(|identity| !identity.valid(None))
            || self
                ._source_dependency_closure_identity
                .as_ref()
                .is_some_and(|identity| !identity.valid(None))
        {
            return Err("transition-pose evaluation has an unsupported V1 shape".to_owned());
        }
        Ok(())
    }
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
    translation_offenders: Vec<TranslationOffenderWire>,
    rotation_offenders: Vec<RotationOffenderWire>,
}

impl TransitionPosePairWire {
    fn validate(&self, members: usize) -> Result<(), String> {
        if self
            .member_indices
            .iter()
            .any(|index| *index >= members as u64)
            || !matches!(self.boundary.as_str(), "entry" | "exit")
            || self.translation_offenders.len() > 16
            || self.rotation_offenders.len() > 16
        {
            return Err("transition-pose evaluation has an unsupported pair".to_owned());
        }
        if !self._max_translation_delta_m.is_finite()
            || self._max_translation_delta_m < 0.0
            || !self._max_rotation_delta_deg.is_finite()
            || self._max_rotation_delta_deg < 0.0
            || !self._translation_tolerance_m.is_finite()
            || self._translation_tolerance_m < 0.0
            || !self._rotation_tolerance_deg.is_finite()
            || self._rotation_tolerance_deg < 0.0
        {
            return Err("transition-pose evaluation has non-finite pair values".to_owned());
        }
        for offender in &self.translation_offenders {
            offender.validate()?;
        }
        for offender in &self.rotation_offenders {
            offender.validate()?;
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TranslationOffenderWire {
    bone_ordinal: u64,
    bone_name: String,
    delta_m: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RotationOffenderWire {
    bone_ordinal: u64,
    bone_name: String,
    delta_deg: f64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransitionPoseOffenderWire {
    bone_ordinal: u64,
    bone_name: String,
    delta: f64,
}

impl TransitionPoseOffenderWire {
    fn translation(value: TranslationOffenderWire) -> Self {
        Self {
            bone_ordinal: value.bone_ordinal,
            bone_name: value.bone_name,
            delta: value.delta_m,
        }
    }
    fn rotation(value: RotationOffenderWire) -> Self {
        Self {
            bone_ordinal: value.bone_ordinal,
            bone_name: value.bone_name,
            delta: value.delta_deg,
        }
    }
    fn validate(&self) -> Result<(), String> {
        validate_offender(self.bone_ordinal, self.delta)
    }
}

impl TranslationOffenderWire {
    fn validate(&self) -> Result<(), String> {
        validate_offender(self.bone_ordinal, self.delta_m)
    }
}

impl RotationOffenderWire {
    fn validate(&self) -> Result<(), String> {
        validate_offender(self.bone_ordinal, self.delta_deg)
    }
}

fn validate_offender(bone_ordinal: u64, delta: f64) -> Result<(), String> {
    if bone_ordinal > 4095 || !delta.is_finite() || delta < 0.0 {
        return Err("transition-pose evaluation has invalid offender values".to_owned());
    }
    Ok(())
}

fn valid_transition_reason(reason: &str) -> bool {
    matches!(
        reason,
        "dependency_closure_incomplete"
            | "member_unavailable"
            | "zero_duration"
            | "skeleton_basis_mismatch"
            | "time_tolerance_unsupported"
            | "unsupported_sampling"
            | "input_limit"
            | "family_work_limit"
            | "aggregate_work_limit"
            | "retention_limit"
            | "result_limit"
    )
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
            "reports/%2fescape.html",
            "reports/evil\u{0000}.html",
            "reports/evil\r\n.html",
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
        let value = serde_json::json!({"schema":"urn:animsmith:schema:collection-dashboard:1","schema_version":1,"collection_output":{"sha256":identity,"bytes":0},"summary":{"sources":0,"physical_takes":0,"clips":0,"runtime_sets":0,"findings":0,"unscoped_findings":0,"coverage_gaps":0,"prediction_unavailable":0,"with_findings":0,"evaluated":0,"partial":0,"excluded":0,"unavailable":0,"not_evaluated":0},"view":{"sources":[],"clips":[],"runtime_sets":[]}});
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(validate_authority_readback(&bytes).is_ok());
        let mut mismatch = value.clone();
        mismatch["summary"]["clips"] = 1.into();
        assert!(validate_authority_readback(&serde_json::to_vec(&mismatch).unwrap()).is_err());
        let mut unknown = value;
        unknown["unexpected"] = true.into();
        assert!(validate_authority_readback(&serde_json::to_vec(&unknown).unwrap()).is_err());
    }

    fn rich_authority() -> super::CollectionDashboardAuthorityV1 {
        let identity = serde_json::json!({"sha256": "0".repeat(64), "bytes": 0});
        serde_json::from_value(serde_json::json!({
            "schema":"urn:animsmith:schema:collection-dashboard:1", "schema_version":1,
            "collection_output":identity,
            "summary":{"sources":1,"physical_takes":6,"clips":6,"runtime_sets":1,"findings":2,"unscoped_findings":0,"coverage_gaps":3,"prediction_unavailable":4,"with_findings":1,"evaluated":1,"partial":1,"excluded":1,"unavailable":1,"not_evaluated":1},
            "evaluation":{"input":{"sha256":"0".repeat(64),"bytes":0},"status":"complete","decision":"finding","families":[{"id":"family","status":"complete","decision":"finding","members":[{"take_index":0,"take_name":"Finding","source_input":{"sha256":"0".repeat(64),"bytes":0},"logical_clip":"finding"},{"take_index":1,"take_name":"Partial","source_input":{"sha256":"0".repeat(64),"bytes":0},"logical_clip":"partial"}],"pair_findings":[{"member_indices":[0,1],"boundary":"entry","translation_offenders":[{"bone_ordinal":0,"bone_name":"root","delta":0.25}],"rotation_offenders":[]}]}]},
            "view":{"sources":[{"key":"source","locator":"source.gltf","input":{"sha256":"0".repeat(64),"bytes":0},"availability":"available","loader":"ready","dependency_closure":"complete","takes":[
                {"source_take_index":0,"take_name":"Finding","normalized_clip_index":0,"normalized_clip_name":"Finding","availability":"established","outcome":"with_findings","findings":2,"severities":["error"],"coverage_gaps":3,"prediction_unavailable":4,"coverage":{"complete":1,"partial":0,"excluded":0,"not_evaluated":0}},
                {"source_take_index":1,"take_name":"Partial","normalized_clip_index":1,"normalized_clip_name":"Partial","availability":"established","outcome":"partial","findings":0,"severities":[],"coverage_gaps":0,"prediction_unavailable":0,"coverage":{"complete":0,"partial":1,"excluded":0,"not_evaluated":0}},
                {"source_take_index":2,"take_name":"Evaluated","normalized_clip_index":2,"normalized_clip_name":"Evaluated","availability":"established","outcome":"evaluated","findings":0,"severities":[],"coverage_gaps":0,"prediction_unavailable":0,"coverage":{"complete":1,"partial":0,"excluded":0,"not_evaluated":0}},
                {"source_take_index":3,"take_name":"Excluded","normalized_clip_index":3,"normalized_clip_name":"Excluded","availability":"established","outcome":"excluded","findings":0,"severities":[],"coverage_gaps":0,"prediction_unavailable":0,"coverage":{"complete":0,"partial":0,"excluded":1,"not_evaluated":0}},
                {"source_take_index":4,"take_name":"Unavailable","availability":"normalized_clip_unavailable","outcome":"unavailable","findings":0,"severities":[],"coverage_gaps":0,"prediction_unavailable":0,"coverage":{"complete":0,"partial":0,"excluded":0,"not_evaluated":0}},
                {"source_take_index":5,"take_name":"Not evaluated","normalized_clip_index":5,"normalized_clip_name":"Not evaluated","availability":"established","outcome":"not_evaluated","findings":0,"severities":[],"coverage_gaps":0,"prediction_unavailable":0,"coverage":{"complete":0,"partial":0,"excluded":0,"not_evaluated":1}}
            ],"unscoped_findings":0,"unscoped_severities":[]}],"clips":[
                {"id":"finding","source":"source","take_index":0,"take_name":"Finding","roles":["locomotion"],"availability":"established","outcome":"with_findings","findings":2,"severities":["error"],"coverage_gaps":3,"prediction_unavailable":4,"coverage":{"complete":1,"partial":0,"excluded":0,"not_evaluated":0},"runtime_sets":["set"]},
                {"id":"partial","source":"source","take_index":1,"take_name":"Partial","roles":["locomotion","combat"],"availability":"established","outcome":"partial","findings":0,"severities":[],"coverage_gaps":0,"prediction_unavailable":0,"coverage":{"complete":0,"partial":1,"excluded":0,"not_evaluated":0},"runtime_sets":["set"]},
                {"id":"evaluated","source":"source","take_index":2,"take_name":"Evaluated","roles":[],"availability":"established","outcome":"evaluated","findings":0,"severities":[],"coverage_gaps":0,"prediction_unavailable":0,"coverage":{"complete":1,"partial":0,"excluded":0,"not_evaluated":0},"runtime_sets":[]},
                {"id":"excluded","source":"source","take_index":3,"take_name":"Excluded","roles":[],"availability":"established","outcome":"excluded","findings":0,"severities":[],"coverage_gaps":0,"prediction_unavailable":0,"coverage":{"complete":0,"partial":0,"excluded":1,"not_evaluated":0},"runtime_sets":[]},
                {"id":"unavailable","source":"source","take_index":4,"take_name":"Unavailable","roles":[],"availability":"normalized_clip_unavailable","outcome":"unavailable","findings":0,"severities":[],"coverage_gaps":0,"prediction_unavailable":0,"coverage":{"complete":0,"partial":0,"excluded":0,"not_evaluated":0},"runtime_sets":[]},
                {"id":"not-evaluated","source":"source","take_index":5,"take_name":"Not evaluated","roles":[],"availability":"established","outcome":"not_evaluated","findings":0,"severities":[],"coverage_gaps":0,"prediction_unavailable":0,"coverage":{"complete":0,"partial":0,"excluded":0,"not_evaluated":1},"runtime_sets":[]}],"runtime_sets":[{"id":"set","lifecycle":"complete","members":["finding","partial"],"gaps":["missing_member"]}]}
        }))
        .unwrap()
    }

    #[test]
    fn authority_readback_reconciles_every_outcome_and_relationship() {
        let authority = rich_authority();
        let bytes = serde_json::to_vec(&authority).unwrap();
        assert!(validate_authority_readback(&bytes).is_ok());
        let parsed: super::CollectionDashboardAuthorityV1 = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(serde_json::to_vec(&parsed).unwrap(), bytes);

        let mut unknown_source = rich_authority();
        unknown_source.view.clips[0].source = "missing".to_owned();
        assert!(
            validate_authority_readback(&serde_json::to_vec(&unknown_source).unwrap()).is_err()
        );
        let mut wrong_take_index = rich_authority();
        wrong_take_index.view.clips[0].take_index = 99;
        assert!(
            validate_authority_readback(&serde_json::to_vec(&wrong_take_index).unwrap()).is_err()
        );
        let mut wrong_take_name = rich_authority();
        wrong_take_name.view.clips[0].take_name = "Wrong".to_owned();
        assert!(
            validate_authority_readback(&serde_json::to_vec(&wrong_take_name).unwrap()).is_err()
        );
        let mut forged_nonestablished_availability = rich_authority();
        forged_nonestablished_availability.view.clips[0].availability = "forged".to_owned();
        assert!(
            validate_authority_readback(
                &serde_json::to_vec(&forged_nonestablished_availability).unwrap()
            )
            .is_err()
        );
        let mut missing_physical_take = rich_authority();
        missing_physical_take.view.sources[0].takes.remove(0);
        missing_physical_take.summary.physical_takes = 5;
        missing_physical_take.summary.findings = 0;
        missing_physical_take.summary.coverage_gaps = 0;
        missing_physical_take.summary.prediction_unavailable = 0;
        assert!(
            validate_authority_readback(&serde_json::to_vec(&missing_physical_take).unwrap())
                .is_err()
        );
        let mut mismatched_logical_evidence = rich_authority();
        mismatched_logical_evidence.view.clips[0].findings = 1;
        assert!(
            validate_authority_readback(&serde_json::to_vec(&mismatched_logical_evidence).unwrap())
                .is_err()
        );
        let mut mismatched_logical_outcome = rich_authority();
        let clip = &mut mismatched_logical_outcome.view.clips[0];
        clip.findings = 0;
        clip.severities.clear();
        clip.coverage_gaps = 0;
        clip.prediction_unavailable = 0;
        clip.outcome = "evaluated".to_owned();
        mismatched_logical_outcome.summary.with_findings = 0;
        mismatched_logical_outcome.summary.evaluated = 2;
        assert!(
            validate_authority_readback(&serde_json::to_vec(&mismatched_logical_outcome).unwrap())
                .is_err()
        );
        let mut asymmetric_membership = rich_authority();
        asymmetric_membership.view.clips[0].runtime_sets.clear();
        assert!(
            validate_authority_readback(&serde_json::to_vec(&asymmetric_membership).unwrap())
                .is_err()
        );
        let mut dangling_resolution = rich_authority();
        dangling_resolution.evaluation.as_mut().unwrap().families[0].members[0].logical_clip =
            Some("missing".to_owned());
        assert!(
            validate_authority_readback(&serde_json::to_vec(&dangling_resolution).unwrap())
                .is_err()
        );
        let mut absent_unique_resolution = rich_authority();
        absent_unique_resolution
            .evaluation
            .as_mut()
            .unwrap()
            .families[0]
            .members[0]
            .logical_clip = None;
        assert!(
            validate_authority_readback(&serde_json::to_vec(&absent_unique_resolution).unwrap())
                .is_err()
        );
        let witness_mutations: [fn(&mut super::EvaluationMemberV1); 4] = [
            |member: &mut super::EvaluationMemberV1| member.source_input = None,
            |member: &mut super::EvaluationMemberV1| {
                member.source_input.as_mut().unwrap().bytes = 1
            },
            |member: &mut super::EvaluationMemberV1| member.take_index = 99,
            |member: &mut super::EvaluationMemberV1| member.take_name = "Wrong".to_owned(),
        ];
        for mutate in witness_mutations {
            let mut mismatched_witness = rich_authority();
            mutate(&mut mismatched_witness.evaluation.as_mut().unwrap().families[0].members[0]);
            assert!(
                validate_authority_readback(&serde_json::to_vec(&mismatched_witness).unwrap())
                    .is_err()
            );
        }
        let mut ambiguous_resolution = rich_authority();
        let mut duplicate = ambiguous_resolution.view.clips[0].clone();
        duplicate.id = "finding-alias".to_owned();
        ambiguous_resolution.view.clips.push(duplicate);
        ambiguous_resolution.view.runtime_sets[0]
            .members
            .push("finding-alias".to_owned());
        ambiguous_resolution.summary.clips += 1;
        ambiguous_resolution.summary.with_findings += 1;
        assert!(
            validate_authority_readback(&serde_json::to_vec(&ambiguous_resolution).unwrap())
                .is_err()
        );
        ambiguous_resolution.evaluation.as_mut().unwrap().families[0].members[0].logical_clip =
            None;
        assert!(
            validate_authority_readback(&serde_json::to_vec(&ambiguous_resolution).unwrap())
                .is_ok()
        );
        let mut bad_severity = rich_authority();
        bad_severity.view.clips[0].severities = vec!["fatal".to_owned()];
        assert!(validate_authority_readback(&serde_json::to_vec(&bad_severity).unwrap()).is_err());

        let mut incomplete_physical_identity = rich_authority();
        incomplete_physical_identity.view.sources[0].takes[0].normalized_clip_name = None;
        assert!(
            validate_authority_readback(
                &serde_json::to_vec(&incomplete_physical_identity).unwrap()
            )
            .is_err()
        );
        let mut contradictory_physical_outcome = rich_authority();
        contradictory_physical_outcome.view.sources[0].takes[0].outcome = "evaluated".to_owned();
        assert!(
            validate_authority_readback(
                &serde_json::to_vec(&contradictory_physical_outcome).unwrap()
            )
            .is_err()
        );
        let mut false_duplicate_reason = rich_authority();
        false_duplicate_reason.view.sources[0].takes[1].availability =
            "duplicate_normalized_clip_name".to_owned();
        assert!(
            validate_authority_readback(&serde_json::to_vec(&false_duplicate_reason).unwrap())
                .is_err()
        );
        let mut hidden_duplicate_name = rich_authority();
        hidden_duplicate_name.view.sources[0].takes[1].normalized_clip_name =
            Some("Finding".to_owned());
        assert!(
            validate_authority_readback(&serde_json::to_vec(&hidden_duplicate_name).unwrap())
                .is_err()
        );

        let mut missing_source_identity = rich_authority();
        missing_source_identity.view.sources[0].input = None;
        assert!(
            validate_authority_readback(&serde_json::to_vec(&missing_source_identity).unwrap())
                .is_err()
        );
        let mut orphaned_unscoped_severity = rich_authority();
        orphaned_unscoped_severity.view.sources[0].unscoped_severities = vec!["error".to_owned()];
        assert!(
            validate_authority_readback(&serde_json::to_vec(&orphaned_unscoped_severity).unwrap())
                .is_err()
        );
        let mut unscoped_finding = rich_authority();
        unscoped_finding.view.sources[0].unscoped_findings = 1;
        unscoped_finding.view.sources[0].unscoped_severities = vec!["warning".to_owned()];
        unscoped_finding.summary.findings = 3;
        unscoped_finding.summary.unscoped_findings = 1;
        assert!(
            validate_authority_readback(&serde_json::to_vec(&unscoped_finding).unwrap()).is_ok()
        );
    }

    #[test]
    fn availability_outcomes_match_dashboard_input_check_reference_mapping() {
        assert_eq!(
            super::derive_evidence_outcome(
                0,
                &super::DashboardCoverageV1::default(),
                "established"
            ),
            "not_evaluated"
        );
        for availability in ["duplicate_embedded_take_name", "nested_output_unavailable"] {
            assert_eq!(
                super::derive_evidence_outcome(
                    0,
                    &super::DashboardCoverageV1::default(),
                    availability
                ),
                "unavailable"
            );
            let mut authority = rich_authority();
            authority.view.clips[4].availability = availability.to_owned();
            assert!(validate_authority_readback(&serde_json::to_vec(&authority).unwrap()).is_ok());
        }
    }

    #[test]
    fn transition_reader_rejects_closure_and_delta_cross_contract_mutations() {
        let identity = serde_json::json!({"sha256": "0".repeat(64), "bytes": 0});
        let value = serde_json::json!({
            "schema":"urn:animsmith:schema:transition-pose-evaluation:1", "schema_version":1,
            "status":"incomplete", "decision":"not_evaluated",
            "declaration_input":identity, "declaration_normalized":{"sha256":"0".repeat(64),"bytes":0}, "subject_input":{"sha256":"0".repeat(64),"bytes":0},
            "families":[{"family_id":"family","status":"incomplete","decision":"not_evaluated","reason":"dependency_closure_incomplete","members":[
                {"take_index":0,"take_name":"a","source_input":{"sha256":"0".repeat(64),"bytes":0},"source_dependency_closure_identity":{"sha256":"0".repeat(64),"bytes":0}},
                {"take_index":1,"take_name":"b","source_input":{"sha256":"0".repeat(64),"bytes":0},"source_dependency_closure_identity":{"sha256":"0".repeat(64),"bytes":0}}],"pairs":[]}]
        });
        let wire: super::TransitionPoseEvaluationWire = serde_json::from_value(value).unwrap();
        assert!(wire.validate().is_err());

        let missing_closure = serde_json::json!({
            "schema":"urn:animsmith:schema:transition-pose-evaluation:1", "schema_version":1,
            "status":"incomplete", "decision":"not_evaluated",
            "declaration_input":{"sha256":"0".repeat(64),"bytes":0}, "declaration_normalized":{"sha256":"0".repeat(64),"bytes":0}, "subject_input":{"sha256":"0".repeat(64),"bytes":0},
            "families":[{"family_id":"family","status":"incomplete","decision":"not_evaluated","reason":"zero_duration","members":[
                {"take_index":0,"take_name":"a","source_input":{"sha256":"0".repeat(64),"bytes":0}},
                {"take_index":1,"take_name":"b","source_input":{"sha256":"0".repeat(64),"bytes":0},"source_dependency_closure_identity":{"sha256":"0".repeat(64),"bytes":0}}],"pairs":[]}]
        });
        let wire: super::TransitionPoseEvaluationWire =
            serde_json::from_value(missing_closure).unwrap();
        assert!(wire.validate().is_err());

        let pair = serde_json::json!({"member_indices":[0,1],"boundary":"entry","max_translation_delta_m":0.0,"max_rotation_delta_deg":0.0,"translation_tolerance_m":0.0,"rotation_tolerance_deg":0.0,"translation_offenders":[{"bone_ordinal":0,"bone_name":"bone","delta":0.0}],"rotation_offenders":[]});
        assert!(serde_json::from_value::<super::TransitionPosePairWire>(pair).is_err());

        let dashboard_pair = serde_json::json!({"member_indices":[0,1],"boundary":"entry","translation_offenders":[{"bone_ordinal":0,"bone_name":"bone","delta":0.0}],"rotation_offenders":[]});
        let dashboard_pair: super::EvaluationPairFindingV1 =
            serde_json::from_value(dashboard_pair).unwrap();
        assert!(dashboard_pair.validate(2).is_ok());
    }
}
