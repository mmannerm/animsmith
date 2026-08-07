//! Artifact-level proof for the rest/bind reparameterization.
//!
//! [`animsmith_core::scale::prove_scale`] carries every claim the normalized
//! model can state — preserved world joint translations and orientations,
//! sampled trajectories and derived velocities, the skin equation `W * B`,
//! skinned bounds, unit composed scale at every affected node, and the
//! complete expected world affine of every transform-only attachment. This
//! layer runs it on the reloaded artifact and then proves what that model
//! cannot see:
//!
//! 1. every raw source payload the normalized document does not model;
//! 2. byte preservation of every buffer byte outside the rewritten accessor
//!    ranges — which is also what proves mesh `POSITION`, normals, `TANGENT`,
//!    `COLOR_n`, secondary influences, key times, and every rotation and
//!    scale sampler come through byte-for-byte, since #280 has already proved
//!    those ranges disjoint from every scale-bearing accessor;
//! 3. array identities, plus honest reporting: the accessor indices and JSON
//!    pointers the artifact says it rewrote are exactly the ones this
//!    module's own scan derives;
//! 4. that every rewritten element is the single narrowing of `before * m`
//!    for the multiplier §D.2 requires of *that* element's domain, and that
//!    every element inside a rewritten accessor whose multiplier is one is
//!    bit-identical;
//! 5. determinism, container integrity, and `min`/`max` consistency.
//!
//! # Why the domain is derived twice, by two different routes
//!
//! [`super::rest_bind::rewrite_rest_bind`] derives its closure by walking the
//! raw `/nodes/*/children` arrays. This module derives it from
//! [`ScalePlan::affected_nodes`] resolved through the loader's
//! `SourceNodeAsset::bone` projection, and derives every node's `s_parent`
//! from `Skeleton::parent` rather than from the raw child arrays. So the two
//! agree only when the raw hierarchy, the source-node projection and the
//! normalized skeleton all describe the same tree. `animsmith_core`'s
//! document-shape validation covers the second and third of those — under
//! `Complete` coverage, which rest/bind planning requires, a projection that
//! contradicts its skeleton is refused by `plan_scale`. The raw child arrays
//! are the description it cannot reach: they live in JSON that never becomes
//! a `Document` field, so their agreement is checked only in this crate.
//!
//! The accessor scan below is likewise a second implementation of §D.2's
//! domain table rather than a call into the rewriter's. A proof that asks the
//! implementation which locations it decided to change cannot fail when the
//! implementation decides wrongly.

use super::bytes::{self, AccessorSpan};
use super::proof::{
    GltfScaleArtifactProof, check_array_identities, check_container_integrity,
    check_preserved_bytes, collect_json_differences, failed, numeric, object, track_dimensionless,
    track_length,
};
use super::rest_bind::{RestBindComponents, RestBindDomain, rewrite_rest_bind};
use super::{GltfScaleArtifact, GltfScaleRewriteError};
use crate::capability::{GltfScaleSource, declared, raw_json_bytes};
use crate::{LoadError, load_bytes, resolve_buffers};
use animsmith_core::scale::{
    ScaleCandidate, ScaleOperation, ScalePlan, ScaleTolerancePolicy, prove_scale,
};
use animsmith_core::{BoneId, Document};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// One node transform member, its authored arity, and the multiplier each of
/// its entries must have received.
type NodeMemberExpectation<'a> = (&'static str, usize, &'a dyn Fn(usize) -> f64);

/// One accessor's independently derived rebase expectation.
#[derive(Debug, Clone, PartialEq)]
struct ExpectedRebase {
    components: RestBindComponents,
    /// One multiplier per accessor element.
    element_factors: Vec<f64>,
}

impl ExpectedRebase {
    fn multiplier(&self, element: usize, component: usize) -> f64 {
        if !self.components.covers(component) {
            return 1.0;
        }
        self.element_factors.get(element).copied().unwrap_or(1.0)
    }

    fn is_identity(&self) -> bool {
        self.element_factors.iter().all(|&factor| factor == 1.0)
    }

    fn required_accessor_type(&self) -> Option<&'static str> {
        match self.components {
            RestBindComponents::All => None,
            RestBindComponents::Mat4Rows => Some("MAT4"),
        }
    }
}

/// Independently re-derive and check every artifact-level claim of the
/// rest/bind reparameterization.
///
/// # Errors
///
/// Returns [`GltfScaleRewriteError::ArtifactProofFailed`] for the first
/// failed claim, [`GltfScaleRewriteError::Plan`] when the reloaded candidate
/// fails the shared core proof, and [`GltfScaleRewriteError::Load`] when the
/// artifact cannot be re-read.
pub fn prove_rewritten_rest_bind(
    source: &GltfScaleSource,
    artifact: &GltfScaleArtifact,
    plan: &ScalePlan,
) -> Result<GltfScaleArtifactProof, GltfScaleRewriteError> {
    let tolerance = plan.tolerance_policy();
    let ScaleOperation::RestBindUniformScale {
        source_skin_index,
        source_root_node_index,
        expected_factor,
    } = plan.operation()
    else {
        return Err(failed(
            "plan declares a rest/bind reparameterization",
            1.0,
            0.0,
        ));
    };
    // The artifact carries its own operation, so a rest/bind artifact cannot
    // be proved against a plan that merely happens to share its factor: the
    // two operations rewrite different domains.
    if plan.operation() != artifact.operation() {
        return Err(failed(
            "plan operation equals the artifact's declared operation",
            1.0,
            0.0,
        ));
    }

    let reloaded = load_bytes(Path::new("scale-artifact"), artifact.bytes())?;
    let core = prove_scale(
        source.document(),
        &ScaleCandidate::from_document(reloaded),
        plan,
    )
    .map_err(GltfScaleRewriteError::Plan)?;

    let (container, json_bytes) = raw_json_bytes(artifact.bytes())?;
    if container != artifact.container() {
        return Err(failed("artifact container kind is unchanged", 1.0, 0.0));
    }
    let artifact_json: Value = serde_json::from_slice(json_bytes)
        .map_err(|error| LoadError::Malformed(format!("artifact JSON is invalid: {error}")))?;
    let source_root = object(source.raw_json())?;
    let artifact_root = object(&artifact_json)?;
    let artifact_gltf = gltf::Gltf::from_slice(artifact.bytes()).map_err(LoadError::Gltf)?;
    let artifact_buffers = resolve_buffers(&artifact_gltf, None)?;

    check_container_integrity(artifact, artifact_root, &artifact_buffers)?;
    check_array_identities(source_root, artifact_root)?;

    let domain = domain_from_skeleton(
        source.document(),
        plan,
        source_root_node_index,
        expected_factor,
    )?;

    let mut proof = GltfScaleArtifactProof {
        core,
        length_factor_residual: 0.0,
        dimensionless_residual: 0.0,
        preserved_byte_ranges: 0,
        rewritten_accessor_count: 0,
    };
    let mut rewritten_pointers = BTreeSet::new();
    check_node_transforms(
        source_root,
        artifact_root,
        &domain,
        &tolerance,
        &mut rewritten_pointers,
        &mut proof,
    )?;

    let expected = expected_rebases(source_root, &domain, source_skin_index)?;
    let rewritten: BTreeMap<usize, ExpectedRebase> = expected
        .into_iter()
        .filter(|(_, rebase)| !rebase.is_identity())
        .collect();
    if artifact.rewritten_accessors() != rewritten.keys().copied().collect::<Vec<_>>() {
        return Err(failed(
            "artifact reports exactly the accessors this proof independently derives",
            artifact.rewritten_accessors().len() as f64,
            rewritten.len() as f64,
        ));
    }
    proof.rewritten_accessor_count = rewritten.len();

    let mut spans = Vec::with_capacity(rewritten.len());
    for (&accessor_index, rebase) in &rewritten {
        let required = rebase.required_accessor_type();
        let span = bytes::accessor_span_typed(
            source_root,
            source.resolved_buffers(),
            accessor_index,
            required,
        )?;
        let artifact_span =
            bytes::accessor_span_typed(artifact_root, &artifact_buffers, accessor_index, required)?;
        if span != artifact_span {
            return Err(failed(
                "rewritten accessors keep their source byte layout",
                artifact_span.start as f64,
                span.start as f64,
            ));
        }
        spans.push(span);
    }

    check_rewritten_payloads(
        source.resolved_buffers(),
        &artifact_buffers,
        &spans,
        &rewritten,
        &tolerance,
        &mut proof,
    )?;
    check_accessor_bounds(
        source_root,
        artifact_root,
        &artifact_buffers,
        &spans,
        &rewritten,
        &tolerance,
        &mut rewritten_pointers,
        &mut proof,
    )?;

    if artifact.rewritten_json_pointers() != rewritten_pointers.iter().cloned().collect::<Vec<_>>()
    {
        return Err(failed(
            "artifact reports exactly the JSON pointers this proof independently derives",
            artifact.rewritten_json_pointers().len() as f64,
            rewritten_pointers.len() as f64,
        ));
    }

    let mut allowed = rewritten_pointers;
    for &buffer_index in artifact.reencoded_buffers() {
        allowed.insert(format!("/buffers/{buffer_index}/uri"));
    }
    let mut differences = Vec::new();
    collect_json_differences(
        source.raw_json(),
        &artifact_json,
        "",
        &allowed,
        &mut differences,
    );
    if !differences.is_empty() {
        return Err(failed(
            "every raw JSON location outside the rewritten set is preserved exactly",
            differences.len() as f64,
            0.0,
        ));
    }

    proof.preserved_byte_ranges =
        check_preserved_bytes(source.resolved_buffers(), &artifact_buffers, &spans)?;

    let repeat = rewrite_rest_bind(
        source,
        source_skin_index,
        source_root_node_index,
        expected_factor,
    )?;
    if repeat.bytes() != artifact.bytes() {
        return Err(failed(
            "rewriting the same source twice yields identical bytes",
            repeat.bytes().len() as f64,
            artifact.bytes().len() as f64,
        ));
    }
    Ok(proof)
}

// --- Independent domain derivation ------------------------------------------

/// Rebuild the affected domain from the plan's bone-keyed closure and the
/// normalized skeleton's parent links.
fn domain_from_skeleton(
    document: &Document,
    plan: &ScalePlan,
    source_root_node_index: usize,
    factor: f64,
) -> Result<RestBindDomain, GltfScaleRewriteError> {
    let mut source_of_bone: BTreeMap<BoneId, usize> = BTreeMap::new();
    for node in &document.assets.source_skeleton.nodes {
        if let Some(bone) = node.bone
            && source_of_bone
                .insert(bone, node.source_node_index)
                .is_some()
        {
            return Err(GltfScaleRewriteError::AmbiguousSourceNodeProjection { bone });
        }
    }
    let affected: BTreeSet<BoneId> = plan.affected_nodes().iter().copied().collect();
    let mut closure = BTreeSet::new();
    for &bone in &affected {
        let source = *source_of_bone
            .get(&bone)
            .ok_or(GltfScaleRewriteError::AmbiguousSourceNodeProjection { bone })?;
        closure.insert(source);
    }
    if !closure.contains(&source_root_node_index) {
        return Err(failed(
            "the declared root selector is inside the plan's affected closure",
            0.0,
            1.0,
        ));
    }
    // The structural fact §D.2's rewrite rests on, re-derived here from
    // `Skeleton::parent` rather than from the raw child arrays the rewriter
    // walked: exactly one affected node has a parent outside the closure, and
    // it is the declared root. Without it "the root's local scale is the only
    // scale that changes" would be an assumption rather than a checked fact.
    let mut boundary = Vec::new();
    for &bone in &affected {
        let parent = document
            .skeleton
            .bones
            .get(bone)
            .ok_or(GltfScaleRewriteError::UnusableSourceHierarchy {
                reason: "an affected bone is out of range for the loaded skeleton",
            })?
            .parent;
        if !parent.is_some_and(|parent| affected.contains(&parent)) {
            boundary.push(source_of_bone[&bone]);
        }
    }
    if boundary != vec![source_root_node_index] {
        return Err(failed(
            "exactly one affected node has a parent outside the closure, and it is the declared root",
            boundary.len() as f64,
            1.0,
        ));
    }
    Ok(RestBindDomain {
        root: source_root_node_index,
        closure,
        factor,
    })
}

/// Every accessor §D.2 rebases, and by how much, derived here by this
/// module's own scan of the raw JSON.
fn expected_rebases(
    root: &Map<String, Value>,
    domain: &RestBindDomain,
    source_skin_index: usize,
) -> Result<BTreeMap<usize, ExpectedRebase>, GltfScaleRewriteError> {
    let index = |value: Option<&Value>| -> Option<usize> {
        value?.as_u64().and_then(|index| index.try_into().ok())
    };
    fn array(value: Option<&Value>) -> &[Value] {
        value
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice)
    }
    let count = |accessor_index: usize| -> Result<usize, GltfScaleRewriteError> {
        array(root.get("accessors"))
            .get(accessor_index)
            .and_then(|accessor| accessor.get("count"))
            .and_then(Value::as_u64)
            .and_then(|count| usize::try_from(count).ok())
            .ok_or(GltfScaleRewriteError::UnrewritableAccessor {
                accessor_index,
                location: format!("/accessors/{accessor_index}"),
            })
    };
    // The selected skin must exist: the plan resolved it, and a proof that
    // silently skipped it would never derive the inverse-bind expectation
    // that is this operation's whole point.
    if array(root.get("skins")).get(source_skin_index).is_none() {
        return Err(failed(
            "the declared skin selector names a source skin",
            0.0,
            1.0,
        ));
    }

    let mut out: BTreeMap<usize, ExpectedRebase> = BTreeMap::new();
    let mut record = |accessor_index: usize, rebase: ExpectedRebase| {
        // First claimant wins — and this scan visits meshes, then skins, then
        // animations, which is exactly `collect_rest_bind_claims`'s order. So
        // on a *shared* accessor this is not a second opinion: it necessarily
        // keeps the claimant the rewriter kept, and would agree with a wrong
        // choice as readily as with a right one.
        //
        // What this scan is independent about is *which* accessors are
        // rebased and by *what* factors: both are re-derived here from the
        // raw JSON and §D.2's table rather than read back from the rewriter.
        // Conflict resolution is not in that set. It rests entirely on
        // `ConflictingRestBindFactor` having fired first — `rewrite_rest_bind`
        // refuses every source whose claimants disagree, so no artifact
        // reaches this proof needing an accessor arbitrated at all. That is a
        // dependency on the rewriter, stated here rather than assumed, and
        // the four refusal fixtures in `tests/scale_rest_bind.rs` are what
        // hold it up.
        out.entry(accessor_index).or_insert(rebase);
    };

    for mesh in array(root.get("meshes")) {
        for primitive in array(mesh.get("primitives")) {
            let Some(accessor_index) = index(
                primitive
                    .get("attributes")
                    .and_then(|attributes| attributes.get("POSITION")),
            ) else {
                continue;
            };
            record(
                accessor_index,
                ExpectedRebase {
                    components: RestBindComponents::All,
                    element_factors: vec![1.0; count(accessor_index)?],
                },
            );
        }
    }
    for skin in array(root.get("skins")) {
        let Some(accessor_index) = index(skin.get("inverseBindMatrices")) else {
            continue;
        };
        let factors: Vec<f64> = array(skin.get("joints"))
            .iter()
            .map(|joint| match index(Some(joint)) {
                Some(joint) => domain.node_factor(joint),
                None => 1.0,
            })
            .collect();
        if factors.len() != count(accessor_index)? {
            return Err(failed(
                "a skin's inverse-bind accessor has one matrix per declared joint",
                factors.len() as f64,
                count(accessor_index)? as f64,
            ));
        }
        record(
            accessor_index,
            ExpectedRebase {
                components: RestBindComponents::Mat4Rows,
                element_factors: factors,
            },
        );
    }
    for animation in array(root.get("animations")) {
        let samplers = array(animation.get("samplers"));
        for channel in array(animation.get("channels")) {
            let target = channel.get("target");
            if target
                .and_then(|target| target.get("path"))
                .and_then(Value::as_str)
                != Some("translation")
            {
                continue;
            }
            let Some(node) = index(target.and_then(|target| target.get("node"))) else {
                continue;
            };
            let Some(accessor_index) = index(channel.get("sampler"))
                .and_then(|sampler| samplers.get(sampler))
                .and_then(|sampler| index(sampler.get("output")))
            else {
                continue;
            };
            record(
                accessor_index,
                ExpectedRebase {
                    components: RestBindComponents::All,
                    element_factors: vec![domain.parent_factor(node); count(accessor_index)?],
                },
            );
        }
    }
    Ok(out)
}

// --- Claims -----------------------------------------------------------------

/// Every node transform member: rebased ones by their analytic multiplier,
/// every other one unchanged.
fn check_node_transforms(
    source_root: &Map<String, Value>,
    artifact_root: &Map<String, Value>,
    domain: &RestBindDomain,
    tolerance: &ScaleTolerancePolicy,
    rewritten_pointers: &mut BTreeSet<String>,
    proof: &mut GltfScaleArtifactProof,
) -> Result<(), GltfScaleRewriteError> {
    let nodes = |root: &Map<String, Value>| -> Vec<Value> {
        root.get("nodes")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
    };
    let source_nodes = nodes(source_root);
    let artifact_nodes = nodes(artifact_root);
    for (node_index, (before_node, after_node)) in
        source_nodes.iter().zip(&artifact_nodes).enumerate()
    {
        let s_parent = domain.parent_factor(node_index);
        let s_local = domain.local_scale_factor(node_index);
        // Column-major `matrix`: the nine linear entries take `s_parent /
        // s_i`, the three translation entries take `s_parent`, and the
        // homogeneous row is invariant.
        let expectations: [NodeMemberExpectation<'_>; 3] = [
            ("translation", 3, &|_| s_parent),
            ("scale", 3, &|_| s_local),
            ("matrix", 16, &|component| match component {
                0..=2 | 4..=6 | 8..=10 => s_local,
                12..=14 => s_parent,
                _ => 1.0,
            }),
        ];
        for (member, length, multiplier) in expectations {
            let declared_before = declared(before_node, member);
            let declared_after = declared(after_node, member);
            let (Some(before_values), Some(after_values)) = (declared_before, declared_after)
            else {
                // A member absent from both sides is untouched. A `scale`
                // that only the artifact declares is the materialized glTF
                // default `[1, 1, 1] * s_local`, which is the one location
                // this operation is allowed to add.
                if declared_before.is_none()
                    && let Some(after_values) = declared_after
                {
                    if member != "scale" || s_local == 1.0 {
                        return Err(failed(
                            "the artifact declares no node transform member the source did not",
                            1.0,
                            0.0,
                        ));
                    }
                    for component in 0..length {
                        let after = numeric(
                            after_values
                                .get(component)
                                .ok_or_else(|| missing(node_index, member))?,
                            member,
                        )?;
                        track_length(1.0, after, s_local, tolerance, proof)?;
                    }
                    rewritten_pointers.insert(format!("/nodes/{node_index}/{member}"));
                } else if declared_after.is_none() && declared_before.is_some() {
                    return Err(failed(
                        "the artifact keeps every node transform member the source declared",
                        0.0,
                        1.0,
                    ));
                }
                continue;
            };
            let mut changed = false;
            for component in 0..length {
                let before = numeric(
                    before_values
                        .get(component)
                        .ok_or_else(|| missing(node_index, member))?,
                    member,
                )?;
                let after = numeric(
                    after_values
                        .get(component)
                        .ok_or_else(|| missing(node_index, member))?,
                    member,
                )?;
                let factor = multiplier(component);
                if factor == 1.0 {
                    track_dimensionless(before, after, tolerance, proof)?;
                } else {
                    track_length(before, after, factor, tolerance, proof)?;
                    changed = true;
                }
            }
            if changed {
                rewritten_pointers.insert(format!("/nodes/{node_index}/{member}"));
            }
        }
        // Node `rotation` is dimensionless under `L' = C_parent^-1 * L * C_i`
        // for uniform `C`, and so are `children`, `mesh`, `skin`, `name` and
        // every other member. None of them is checked here: the byte
        // preservation walk cannot see them — a node's members live in JSON —
        // but `collect_json_differences` can and does, because a location
        // this function does not add to `rewritten_pointers` is not in the
        // allowed set and is reported as a difference. An explicit member
        // list here would be strictly weaker (it would have to enumerate the
        // members) and no mutation could distinguish it, so the general walk
        // is left to own the claim.
    }
    Ok(())
}

fn missing(node_index: usize, member: &str) -> GltfScaleRewriteError {
    LoadError::Malformed(format!("/nodes/{node_index}/{member} is too short")).into()
}

/// Every rewritten `f32` is the single narrowing of `before * m`, and every
/// element inside a rewritten accessor whose multiplier is one is
/// bit-identical.
fn check_rewritten_payloads(
    source_buffers: &[Vec<u8>],
    artifact_buffers: &[Vec<u8>],
    spans: &[AccessorSpan],
    rewritten: &BTreeMap<usize, ExpectedRebase>,
    tolerance: &ScaleTolerancePolicy,
    proof: &mut GltfScaleArtifactProof,
) -> Result<(), GltfScaleRewriteError> {
    for &span in spans {
        let rebase = &rewritten[&span.accessor_index];
        let before = bytes::read_span(source_buffers, span);
        let after = bytes::read_span(artifact_buffers, span);
        if before.len() != after.len() {
            return Err(failed(
                "a rewritten accessor keeps its element count",
                after.len() as f64,
                before.len() as f64,
            ));
        }
        for (index, (&before, &after)) in before.iter().zip(&after).enumerate() {
            let factor = rebase.multiplier(index / span.components, index % span.components);
            if factor == 1.0 {
                if after.to_bits() != before.to_bits() {
                    return Err(failed(
                        "an unrebased element of a rewritten accessor is bit-identical",
                        f64::from(after),
                        f64::from(before),
                    ));
                }
                continue;
            }
            let expected = f64::from(before) * factor;
            if after.to_bits() != (expected as f32).to_bits() {
                return Err(failed(
                    "every rebased element is the single narrowing of before * m",
                    f64::from(after),
                    expected,
                ));
            }
            track_length(
                f64::from(before),
                f64::from(after),
                factor,
                tolerance,
                proof,
            )?;
        }
    }
    Ok(())
}

/// A rewritten accessor's `min`/`max` still bound its rewritten payload.
#[allow(clippy::too_many_arguments)]
fn check_accessor_bounds(
    source_root: &Map<String, Value>,
    artifact_root: &Map<String, Value>,
    artifact_buffers: &[Vec<u8>],
    spans: &[AccessorSpan],
    rewritten: &BTreeMap<usize, ExpectedRebase>,
    tolerance: &ScaleTolerancePolicy,
    rewritten_pointers: &mut BTreeSet<String>,
    proof: &mut GltfScaleArtifactProof,
) -> Result<(), GltfScaleRewriteError> {
    for &span in spans {
        let rebase = &rewritten[&span.accessor_index];
        let payload = bytes::read_span(artifact_buffers, span);
        for (member, is_min) in [("min", true), ("max", false)] {
            let pointer = format!("/accessors/{}/{member}", span.accessor_index);
            let bounds = |root: &Map<String, Value>| -> Option<Vec<Value>> {
                root.get("accessors")
                    .and_then(Value::as_array)
                    .and_then(|accessors| accessors.get(span.accessor_index))
                    .and_then(|accessor| accessor.get(member))
                    .and_then(Value::as_array)
                    .cloned()
            };
            let Some(source_bounds) = bounds(source_root) else {
                continue;
            };
            let artifact_bounds = bounds(artifact_root)
                .filter(|bounds| bounds.len() == source_bounds.len())
                .ok_or_else(|| {
                    failed(
                        "a rewritten accessor keeps its authored bound arity",
                        0.0,
                        source_bounds.len() as f64,
                    )
                })?;
            for component in 0..source_bounds.len() {
                let before = numeric(&source_bounds[component], &pointer)?;
                let after = numeric(&artifact_bounds[component], &pointer)?;
                if !rebase.components.covers(component) {
                    track_dimensionless(before, after, tolerance, proof)?;
                    continue;
                }
                let declared_bound = after as f32;
                let observed = payload
                    .iter()
                    .skip(component)
                    .step_by(span.components)
                    .copied()
                    .fold(
                        if is_min {
                            f32::INFINITY
                        } else {
                            f32::NEG_INFINITY
                        },
                        |accumulator, value| {
                            if is_min {
                                accumulator.min(value)
                            } else {
                                accumulator.max(value)
                            }
                        },
                    );
                let bounds_data = if is_min {
                    declared_bound <= observed
                } else {
                    declared_bound >= observed
                };
                if !bounds_data {
                    return Err(failed(
                        "a rewritten bound still bounds the rewritten payload",
                        f64::from(declared_bound),
                        f64::from(observed),
                    ));
                }
            }
            rewritten_pointers.insert(pointer);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Per-claim negative tests.
    //!
    //! Each corrupts exactly one thing about an otherwise valid artifact and
    //! asserts the exact claim that must catch it. These live here rather
    //! than in `tests/scale_rest_bind.rs` because
    //! [`super::super::GltfScaleArtifact`]'s fields are private, and four of
    //! the claims — the accessor, pointer and operation cross-checks and the
    //! stale-candidate case — can only be falsified by making the artifact's
    //! own report or its own bytes disagree with what the rewriter produced.
    //!
    //! Two are not corruption tests, and say so where they stand. The
    //! node-member residuals are exercised by calling
    //! [`check_node_transforms`] directly, because every node member this
    //! fixture rebases is also modelled by `prove_scale`, which runs first
    //! and would report its own residual instead. The inverse-bind
    //! joint-count claim is a classification test over a doctored raw tree,
    //! because `rewrite_rest_bind` refuses such a source before an artifact
    //! exists — the same position its mirror in `rest_bind` is in.
    //!
    //! The fixture is the DESIGN.md Appendix D §D.3 case 2 rig of
    //! `tests/scale_rest_bind.rs`, restated here as the smallest thing that
    //! rebases: a root at `0.01`, a joint child authored 100 times too large,
    //! a transform-only grandchild, and a compensating `diag(100)` inverse
    //! bind. Every literal below is that case's arithmetic, not a value this
    //! crate produced.

    use super::*;
    use crate::preflight_scale_source_bytes;
    use animsmith_core::scale::{ProofResidualKind, ScaleError, ScaleRequest, plan_scale};
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use serde_json::json;

    mod at {
        pub const POSITION: usize = 0; // 36
        pub const JOINTS: usize = 36; // 24
        pub const WEIGHTS: usize = 60; // 48
        pub const INVERSE_BIND: usize = 108; // 64
        pub const TIMES: usize = 172; // 8
        pub const TRANSLATION: usize = 180; // 24
        /// Reached by no `bufferView`, so a byte can be flipped outside every
        /// rewritten range without disturbing anything the normalized
        /// `Document` models. Wide enough to also hold a displaced copy of
        /// the translation output, which is what makes the span-layout claim
        /// falsifiable.
        pub const SPARE: usize = 204; // 28
        pub const LENGTH: usize = 232;
    }

    const FACTOR: f64 = 0.01;
    const POSITIONS: [f32; 9] = [0.0, 1.0, 0.0, 0.5, 1.25, -0.25, -0.5, 0.75, 0.5];
    const INVERSE_BIND: [f32; 16] = [
        100.0, 0.0, 0.0, 0.0, //
        0.0, 100.0, 0.0, 0.0, //
        0.0, 0.0, 100.0, 0.0, //
        0.0, -100.0, 0.0, 1.0,
    ];

    fn f32_bytes(values: &[f32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    fn data_uri(bytes: &[u8]) -> String {
        format!(
            "data:application/octet-stream;base64,{}",
            STANDARD.encode(bytes)
        )
    }

    fn fixture_buffer() -> Vec<u8> {
        let mut buffer = vec![0u8; at::LENGTH];
        let mut put = |offset: usize, payload: Vec<u8>| {
            buffer[offset..offset + payload.len()].copy_from_slice(&payload);
        };
        put(at::POSITION, f32_bytes(&POSITIONS));
        put(at::INVERSE_BIND, f32_bytes(&INVERSE_BIND));
        put(at::TIMES, f32_bytes(&[0.0, 1.0]));
        put(
            at::TRANSLATION,
            f32_bytes(&[0.0, 100.0, 0.0, 0.0, 300.0, 0.0]),
        );
        // One joint, full weight on it, for each of the three vertices.
        for vertex in 0..3 {
            let at = at::WEIGHTS + vertex * 16;
            buffer[at..at + 4].copy_from_slice(&1.0f32.to_le_bytes());
        }
        buffer
    }

    /// `animated == false` drops the clip, which is what isolates the
    /// unit-scale postcondition: with a translation track present the
    /// per-element `TrackValue` comparison sees a stale candidate first.
    fn fixture_json(buffer: &[u8], animated: bool) -> Value {
        let mut value = json!({
            "asset": { "version": "2.0" },
            "buffers": [{ "uri": data_uri(buffer), "byteLength": at::LENGTH }],
            "bufferViews": [
                { "buffer": 0, "byteOffset": at::POSITION, "byteLength": 36 },
                { "buffer": 0, "byteOffset": at::JOINTS, "byteLength": 24 },
                { "buffer": 0, "byteOffset": at::WEIGHTS, "byteLength": 48 },
                { "buffer": 0, "byteOffset": at::INVERSE_BIND, "byteLength": 64 },
                { "buffer": 0, "byteOffset": at::TIMES, "byteLength": 8 },
                { "buffer": 0, "byteOffset": at::TRANSLATION, "byteLength": 24 }
            ],
            "accessors": [
                { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                  "min": [-0.5, 0.75, -0.25], "max": [0.5, 1.25, 0.5] },
                { "bufferView": 1, "componentType": 5123, "count": 3, "type": "VEC4" },
                { "bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC4" },
                { "bufferView": 3, "componentType": 5126, "count": 1, "type": "MAT4" },
                { "bufferView": 4, "componentType": 5126, "count": 2, "type": "SCALAR",
                  "min": [0.0], "max": [1.0] },
                { "bufferView": 5, "componentType": 5126, "count": 2, "type": "VEC3" }
            ],
            "materials": [{ "name": "surface" }],
            "meshes": [{ "primitives": [{
                "attributes": { "POSITION": 0, "JOINTS_0": 1, "WEIGHTS_0": 2 },
                "material": 0
            }] }],
            "nodes": [
                { "name": "root", "scale": [0.01, 0.01, 0.01], "children": [1] },
                { "name": "joint", "translation": [0.0, 100.0, 0.0], "children": [2] },
                { "name": "attach", "translation": [1.0, 0.0, 0.0] },
                // The holder is outside the closure, so both its multipliers
                // are one and its authored translation is the fixture's only
                // node member that must come through *invariant* rather than
                // rebased.
                { "name": "holder", "translation": [4.0, 0.0, 0.0], "mesh": 0, "skin": 0 }
            ],
            "scenes": [{ "nodes": [0, 3] }],
            "scene": 0,
            "skins": [{ "joints": [1], "skeleton": 0, "inverseBindMatrices": 3 }]
        });
        if animated {
            value["animations"] = json!([{
                "samplers": [{ "input": 4, "interpolation": "LINEAR", "output": 5 }],
                "channels": [{ "sampler": 0, "target": { "node": 1, "path": "translation" } }]
            }]);
        } else {
            // The sampler accessors would otherwise be payload no accessor
            // reaches, which is fine, but dropping them keeps the preserved
            // byte-range arithmetic below readable.
            value["accessors"] = json!(value["accessors"].as_array().expect("accessors")[..4]);
            value["bufferViews"] = json!(value["bufferViews"].as_array().expect("views")[..4]);
        }
        value
    }

    fn fixture() -> (GltfScaleSource, GltfScaleArtifact, ScalePlan) {
        fixture_with(true)
    }

    fn fixture_with(animated: bool) -> (GltfScaleSource, GltfScaleArtifact, ScalePlan) {
        let value = fixture_json(&fixture_buffer(), animated);
        let bytes = serde_json::to_vec(&value).expect("fixture serializes");
        let source = preflight_scale_source_bytes(Path::new("rest-bind-fixture.gltf"), &bytes)
            .expect("the fixture preflights cleanly");
        let plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: FACTOR,
            },
            document: source.document(),
            capability: &super::super::capability_facts(source.manifest()),
        })
        .expect("plan");
        let artifact = rewrite_rest_bind(&source, 0, 0, FACTOR).expect("rewrite");
        (source, artifact, plan)
    }

    fn artifact_value(artifact: &GltfScaleArtifact) -> Value {
        serde_json::from_slice(artifact.bytes()).expect("a .gltf artifact is JSON")
    }

    fn put_artifact_value(artifact: &mut GltfScaleArtifact, value: &Value) {
        artifact.bytes = serde_json::to_vec(value).expect("corrupted fixture serializes");
    }

    fn artifact_buffer(value: &Value) -> Vec<u8> {
        let uri = value["buffers"][0]["uri"].as_str().expect("data URI");
        STANDARD
            .decode(uri.split_once("base64,").expect("base64 data URI").1)
            .expect("valid base64")
    }

    fn put_artifact_buffer(value: &mut Value, bytes: &[u8]) {
        value["buffers"][0]["uri"] = json!(data_uri(bytes));
    }

    fn expect_claim(
        source: &GltfScaleSource,
        artifact: &GltfScaleArtifact,
        plan: &ScalePlan,
        expected: &str,
    ) {
        match prove_rewritten_rest_bind(source, artifact, plan) {
            Err(GltfScaleRewriteError::ArtifactProofFailed { claim, .. }) => {
                assert_eq!(claim, expected);
            }
            other => panic!("expected the claim {expected:?} to fail, got {other:?}"),
        }
    }

    #[test]
    fn a_skeleton_with_two_closure_boundaries_is_refused() {
        // The structural fact §D.2's rewrite rests on: exactly one affected
        // node has a parent outside the closure, and it is the declared root.
        // No glTF byte sequence can break it — the closure is *built* from
        // the hierarchy — so this is a classification test over a mutated
        // skeleton. Without it the check is a claim nothing evaluates.
        let (source, _, plan) = fixture();
        let mut document = source.document().clone();
        document.skeleton.bones[2].parent = None;
        match domain_from_skeleton(&document, &plan, 0, FACTOR) {
            Err(GltfScaleRewriteError::ArtifactProofFailed {
                claim, observed, ..
            }) => {
                assert_eq!(
                    claim,
                    "exactly one affected node has a parent outside the closure, and it is the declared root"
                );
                assert_eq!(observed, 2.0);
            }
            other => panic!("expected a boundary-count refusal, got {other:?}"),
        }

        // A declared root the plan's closure does not contain is refused too.
        match domain_from_skeleton(source.document(), &plan, 3, FACTOR) {
            Err(GltfScaleRewriteError::ArtifactProofFailed { claim, .. }) => {
                assert_eq!(
                    claim,
                    "the declared root selector is inside the plan's affected closure"
                );
            }
            other => panic!("expected a root-selector refusal, got {other:?}"),
        }

        // The must-not-over-reject direction: the unmutated document derives
        // exactly the §D.2 closure, rooted where the selector says.
        let domain =
            domain_from_skeleton(source.document(), &plan, 0, FACTOR).expect("ordinary document");
        assert_eq!(domain.root, 0);
        assert_eq!(domain.closure, BTreeSet::from([0, 1, 2]));
        assert_eq!(domain.factor, FACTOR);
    }

    #[test]
    fn a_skin_whose_joints_outnumber_its_inverse_binds_fails_the_proofs_own_scan() {
        // The proof's mirror of `collect_rest_bind_claims`'s joint-count
        // guard. The rewriter refuses such a source first, so no artifact
        // carrying it ever reaches `prove_rewritten_rest_bind`; the scan is
        // module-private, so the claim is falsified where it lives rather
        // than left as one nothing evaluates.
        let (source, _, plan) = fixture();
        let domain =
            domain_from_skeleton(source.document(), &plan, 0, FACTOR).expect("ordinary document");
        let mut value = source.raw_json().clone();
        value["skins"][0]["joints"] = json!([1, 2]);
        let root = object(&value).expect("source root").clone();
        match expected_rebases(&root, &domain, 0) {
            Err(GltfScaleRewriteError::ArtifactProofFailed {
                claim,
                observed,
                tolerance,
            }) => {
                assert_eq!(
                    claim,
                    "a skin's inverse-bind accessor has one matrix per declared joint"
                );
                assert_eq!(observed, 2.0, "two declared joints");
                assert_eq!(tolerance, 1.0, "one inverse bind");
            }
            other => panic!("expected a joint-count refusal, got {other:?}"),
        }
        // And the selector must name a skin at all, which is the other way
        // this scan can silently derive no inverse-bind expectation.
        match expected_rebases(&root, &domain, 1) {
            Err(GltfScaleRewriteError::ArtifactProofFailed { claim, .. }) => {
                assert_eq!(claim, "the declared skin selector names a source skin");
            }
            other => panic!("expected a skin-selector refusal, got {other:?}"),
        }
    }

    #[test]
    fn the_uncorrupted_fixture_proves_and_reports_its_evidence() {
        let (source, artifact, plan) = fixture();
        let proof = prove_rewritten_rest_bind(&source, &artifact, &plan).expect("artifact proof");
        assert_eq!(proof.rewritten_accessor_count, 2, "the IBM and the output");
        assert_eq!(proof.length_factor_residual, 0.0);
        assert_eq!(proof.dimensionless_residual, 0.0);
        // 108..172 and 180..204 are rewritten, so 0..108, 172..180 and
        // 204..208 are the preserved complement ranges.
        assert_eq!(proof.preserved_byte_ranges, 3);
        assert_eq!(artifact.rewritten_accessors(), [3, 5]);
        assert_eq!(
            artifact.rewritten_json_pointers(),
            [
                "/nodes/0/scale",
                "/nodes/1/translation",
                "/nodes/2/translation"
            ]
        );
    }

    #[test]
    fn a_stale_no_op_candidate_cannot_pass() {
        // The obligation DESIGN.md §D.6 states as "so a no-op cannot pass",
        // exercised directly: an artifact that is byte-for-byte the source.
        // Every raw-level claim about it is trivially satisfied — nothing
        // changed — so only the composed unit-scale postcondition can reject
        // it, and it must.
        let (source, mut artifact, plan) = fixture_with(false);
        artifact.bytes = source.source_bytes().to_vec();
        match prove_rewritten_rest_bind(&source, &artifact, &plan) {
            Err(GltfScaleRewriteError::Plan(ScaleError::ProofResidualExceeded {
                kind,
                observed,
                tolerance,
            })) => {
                assert_eq!(kind, ProofResidualKind::UnitScale);
                assert_eq!(
                    tolerance,
                    plan.tolerance_policy().postcondition_unit_scale_residual
                );
                // The unrebased composed scale is the source's own `0.01`, so
                // the per-axis residual is `1 - 0.01`, four orders of
                // magnitude above the bound.
                assert!(
                    (observed - (1.0 - f64::from(0.01f32))).abs() < 1e-9,
                    "expected a residual of 1 - s, got {observed}"
                );
            }
            other => panic!("a stale no-op candidate must fail the postcondition, got {other:?}"),
        }

        // With a clip present the same stale candidate is caught earlier
        // still, by the direct per-element track comparison — so the two
        // obligations are not redundant with each other and neither is what
        // the other is standing in for.
        let (source, mut artifact, plan) = fixture_with(true);
        artifact.bytes = source.source_bytes().to_vec();
        match prove_rewritten_rest_bind(&source, &artifact, &plan) {
            Err(GltfScaleRewriteError::Plan(ScaleError::ProofResidualExceeded {
                kind, ..
            })) => {
                assert_eq!(kind, ProofResidualKind::TrackValue);
            }
            other => panic!("an animated stale candidate must fail too, got {other:?}"),
        }
    }

    #[test]
    fn scaling_mesh_position_as_though_this_were_a_unit_conversion_fails() {
        // The domain split that separates the two operations: a rest/bind
        // reparameterization preserves world geometry, so `POSITION` must be
        // byte-identical. Nothing in the normalized model's skin equation
        // notices, because scaling the geometry and the binds together still
        // satisfies `W' * B' == W * B` — only the raw byte-preservation walk
        // does.
        let (source, mut artifact, plan) = fixture();
        let mut value = artifact_value(&artifact);
        let mut buffer = artifact_buffer(&value);
        for index in 0..9 {
            let at = at::POSITION + index * 4;
            let before = f32::from_le_bytes(buffer[at..at + 4].try_into().expect("four bytes"));
            buffer[at..at + 4]
                .copy_from_slice(&((f64::from(before) * FACTOR) as f32).to_le_bytes());
        }
        put_artifact_buffer(&mut value, &buffer);
        put_artifact_value(&mut artifact, &value);
        // Caught by the shared model-level obligation, which states this
        // operation's analytic expectation for base `POSITION` directly as
        // `before`, not by the raw byte walk that would otherwise catch it
        // second. The skin equation alone would *not* catch it: scaling the
        // geometry and the binds together still satisfies `W' * B' == W * B`.
        match prove_rewritten_rest_bind(&source, &artifact, &plan) {
            Err(GltfScaleRewriteError::Plan(ScaleError::ProofResidualExceeded {
                kind,
                observed,
                ..
            })) => {
                assert_eq!(kind, ProofResidualKind::MeshPosition);
                // The *first* element to exceed its bound, not the largest:
                // `prove_scale` reports the residual it stopped on. That is
                // vertex 0's `y`, authored `1.0` and rescaled to `f32(0.01)`.
                assert_eq!(observed, 1.0 - f64::from(0.01f32));
            }
            other => panic!("a rescaled mesh must be refused, got {other:?}"),
        }
    }

    #[test]
    fn a_flipped_byte_outside_every_rewritten_range_fails_byte_preservation() {
        let (source, mut artifact, plan) = fixture();
        let mut value = artifact_value(&artifact);
        let mut buffer = artifact_buffer(&value);
        buffer[at::SPARE] ^= 0xff;
        put_artifact_buffer(&mut value, &buffer);
        put_artifact_value(&mut artifact, &value);
        expect_claim(
            &source,
            &artifact,
            &plan,
            "buffer bytes outside the converted ranges are preserved",
        );
    }

    #[test]
    fn an_unrebased_element_of_a_rewritten_accessor_must_be_bit_identical() {
        // The inverse bind's homogeneous row entry 3 is `+0.0`; setting its
        // sign bit makes it `-0.0`, which is numerically identical and so
        // invisible to every residual and to the reloaded document. Only the
        // bit comparison catches it.
        let (source, mut artifact, plan) = fixture();
        let mut value = artifact_value(&artifact);
        let mut buffer = artifact_buffer(&value);
        const SIGN_BYTE: usize = at::INVERSE_BIND + 3 * 4 + 3;
        buffer[SIGN_BYTE] |= 0x80;
        put_artifact_buffer(&mut value, &buffer);
        put_artifact_value(&mut artifact, &value);
        expect_claim(
            &source,
            &artifact,
            &plan,
            "an unrebased element of a rewritten accessor is bit-identical",
        );
    }

    #[test]
    fn an_under_reported_rewritten_accessor_fails_the_accessor_cross_check() {
        let (source, mut artifact, plan) = fixture();
        artifact.rewritten_accessors.pop();
        expect_claim(
            &source,
            &artifact,
            &plan,
            "artifact reports exactly the accessors this proof independently derives",
        );
    }

    #[test]
    fn an_under_reported_rewritten_json_pointer_fails_the_pointer_cross_check() {
        let (source, mut artifact, plan) = fixture();
        artifact.rewritten_json_pointers.pop();
        expect_claim(
            &source,
            &artifact,
            &plan,
            "artifact reports exactly the JSON pointers this proof independently derives",
        );
    }

    #[test]
    fn a_node_member_the_source_did_not_declare_is_refused() {
        // The materialized root `scale` is the *only* member this operation
        // may add. A `translation` invented on a node that declared none is
        // not, even though its value would be the glTF default rebased.
        let (source, mut artifact, plan) = fixture();
        let mut value = artifact_value(&artifact);
        value["nodes"][0]["translation"] = json!([0.0, 0.0, 0.0]);
        put_artifact_value(&mut artifact, &value);
        expect_claim(
            &source,
            &artifact,
            &plan,
            "the artifact declares no node transform member the source did not",
        );
    }

    #[test]
    fn a_node_member_the_source_declared_and_the_artifact_dropped_is_refused() {
        // The mirror of the test above. The root's rebased `scale` is
        // `[1, 1, 1]`, which is glTF's own default — so dropping the member
        // leaves the reloaded `Document` identical and every model-level
        // obligation satisfied. Only the raw-JSON member comparison can
        // notice that an authored member stopped being authored.
        let (source, mut artifact, plan) = fixture();
        let mut value = artifact_value(&artifact);
        value["nodes"][0]
            .as_object_mut()
            .expect("the root is an object")
            .remove("scale")
            .expect("the artifact declares the rebased root scale");
        put_artifact_value(&mut artifact, &value);
        expect_claim(
            &source,
            &artifact,
            &plan,
            "the artifact keeps every node transform member the source declared",
        );
    }

    #[test]
    fn a_node_member_value_that_is_not_its_multiplier_times_the_source_is_refused() {
        // `check_node_transforms` is exercised directly here rather than
        // through `prove_rewritten_rest_bind`, because every node member this
        // fixture rebases is *also* modelled by `prove_scale` — the root's
        // composed scale, the joint's rest translation, the attachment's
        // world affine — and `prove_scale` runs first, so an end-to-end
        // corruption would be reported as a core residual and this claim
        // would stay unevaluated. The residual accumulator is the uncorrupted
        // run's, so the only thing that differs in each case below is the one
        // doctored member.
        let (source, artifact, plan) = fixture();
        let mut proof =
            prove_rewritten_rest_bind(&source, &artifact, &plan).expect("artifact proof");
        let domain =
            domain_from_skeleton(source.document(), &plan, 0, FACTOR).expect("ordinary document");
        let tolerance = plan.tolerance_policy();
        let source_root = object(source.raw_json()).expect("source root").clone();
        let mut check = |doctored: &Value| -> GltfScaleRewriteError {
            let artifact_root = object(doctored).expect("artifact root").clone();
            check_node_transforms(
                &source_root,
                &artifact_root,
                &domain,
                &tolerance,
                &mut BTreeSet::new(),
                &mut proof,
            )
            .expect_err("a doctored node member must be refused")
        };

        // The joint's authored `(0, 100, 0)` rebases to `100 * s == 1`.
        // Doubling that entry is off by exactly one length unit.
        let mut value = artifact_value(&artifact);
        value["nodes"][1]["translation"] = json!([0.0, 2.0, 0.0]);
        match check(&value) {
            GltfScaleRewriteError::ArtifactProofFailed {
                claim, observed, ..
            } => {
                assert_eq!(
                    claim,
                    "every converted length differs from the source by exactly the declared factor"
                );
                assert_eq!(observed, 1.0, "abs(2 - 100 * s)");
            }
            other => panic!("expected a length-factor residual, got {other:?}"),
        }

        // And the invariant direction: the holder is outside the closure, so
        // its multiplier is one and its translation must come through
        // untouched. Nothing in the analytic table would ever scale it, which
        // is exactly why a proof that skipped the check would look correct.
        let mut value = artifact_value(&artifact);
        value["nodes"][3]["translation"] = json!([4.5, 0.0, 0.0]);
        match check(&value) {
            GltfScaleRewriteError::ArtifactProofFailed {
                claim, observed, ..
            } => {
                assert_eq!(
                    claim,
                    "every dimensionless value inside a converted range is invariant"
                );
                assert_eq!(observed, 0.5, "abs(4.5 - 4)");
            }
            other => panic!("expected a dimensionless residual, got {other:?}"),
        }
    }

    #[test]
    fn a_rewritten_accessor_moved_to_different_bytes_fails_the_span_layout_claim() {
        // The artifact carries the *same* rebased translation values, at a
        // different offset in the same buffer. The reloaded `Document` is
        // therefore identical and every model-level obligation still holds;
        // so does every array identity, since no array changed length. What
        // must reject it is the claim that a rewritten accessor keeps its
        // source byte layout — without which the byte-preservation walk below
        // would be comparing the wrong ranges.
        let (source, mut artifact, plan) = fixture();
        let mut value = artifact_value(&artifact);
        let mut buffer = artifact_buffer(&value);
        let (payload, spare) = (at::TRANSLATION, at::SPARE);
        let displaced: Vec<u8> = buffer[payload..payload + 24].to_vec();
        buffer[spare..spare + 24].copy_from_slice(&displaced);
        put_artifact_buffer(&mut value, &buffer);
        value["bufferViews"][5]["byteOffset"] = json!(spare);
        put_artifact_value(&mut artifact, &value);
        expect_claim(
            &source,
            &artifact,
            &plan,
            "rewritten accessors keep their source byte layout",
        );
    }

    #[test]
    fn an_artifact_whose_declared_operation_differs_from_the_plans_is_refused() {
        // Both are rest/bind reparameterizations, so the `let ... else` that
        // `the_two_operations_cannot_be_proved_against_each_others_plans`
        // exercises does not fire; the selectors disagree, and the two
        // operations rewrite different closures. Only the equality of the
        // whole operation catches it.
        let (source, mut artifact, plan) = fixture();
        artifact.operation = ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 1,
            expected_factor: FACTOR,
        };
        expect_claim(
            &source,
            &artifact,
            &plan,
            "plan operation equals the artifact's declared operation",
        );
    }

    #[test]
    fn a_changed_preserved_json_location_fails_json_preservation() {
        let (source, mut artifact, plan) = fixture();
        let mut value = artifact_value(&artifact);
        value["materials"][0]["name"] = json!("renamed");
        put_artifact_value(&mut artifact, &value);
        expect_claim(
            &source,
            &artifact,
            &plan,
            "every raw JSON location outside the rewritten set is preserved exactly",
        );
    }

    #[test]
    fn bytes_that_are_not_the_rewriters_own_output_fail_the_determinism_claim() {
        // Same JSON value, different bytes. Every claim that reads the
        // artifact through `serde_json` still passes, so nothing but the
        // byte-for-byte repeat comparison can notice.
        let (source, mut artifact, plan) = fixture();
        let value = artifact_value(&artifact);
        artifact.bytes = serde_json::to_vec_pretty(&value).expect("pretty serializes");
        expect_claim(
            &source,
            &artifact,
            &plan,
            "rewriting the same source twice yields identical bytes",
        );
    }

    #[test]
    fn the_two_operations_cannot_be_proved_against_each_others_plans() {
        let (source, artifact, plan) = fixture();
        let whole_document = plan_scale(&ScaleRequest {
            operation: ScaleOperation::WholeDocumentLinearUnits { factor: FACTOR },
            document: source.document(),
            capability: &super::super::capability_facts(source.manifest()),
        })
        .expect("whole-document plan");
        expect_claim(
            &source,
            &artifact,
            &whole_document,
            "plan declares a rest/bind reparameterization",
        );
        // And the mirror, which is why `GltfScaleArtifact` carries its
        // operation rather than only its factor: the two plans agree on the
        // number `0.01` and disagree on everything else.
        match super::super::prove_rewritten_artifact(&source, &artifact, &whole_document) {
            Err(
                GltfScaleRewriteError::Plan(_) | GltfScaleRewriteError::ArtifactProofFailed { .. },
            ) => {}
            other => {
                panic!("a rest/bind artifact must not prove as a unit conversion, got {other:?}")
            }
        }
        let _ = plan;
    }
}
