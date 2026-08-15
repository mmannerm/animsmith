//! Restricted animated rest/bind hierarchy reparameterization of raw
//! glTF/GLB bytes (DESIGN.md Appendix D §D.2, implementation slice 4 of
//! §D.8).
//!
//! [`rewrite_rest_bind`] removes one compensating inherited uniform scale
//! from a closed connected skinned hierarchy, on the source's own JSON tree
//! and its own resolved buffer bytes. Like [`super::rewrite_linear_units`] it
//! never routes through [`crate::write`].
//!
//! # The algebra, and the structural simplification it admits
//!
//! For affected node `i` with basis correction `C_i = scale(1 / s_i)`,
//! §D.2 gives `L_i'(t) = C_parent^-1 * L_i(t) * C_i`. The affected domain is
//! closed and connected and has exactly one node — the closure root — whose
//! parent lies outside it, because the closure is built as "the root, every
//! selected joint, the ancestor path from each joint up to the root, and
//! every descendant of all of those". So `s_parent` is `1` at the root and
//! `s` everywhere else, and the rewrite collapses to:
//!
//! | domain | root | every other affected node |
//! |---|---|---|
//! | node-local `translation` | unchanged | `* s` |
//! | node-local `scale` | `* 1/s` | unchanged |
//! | node `matrix` linear part | `* 1/s` | unchanged |
//! | node `matrix` translation column | unchanged | `* s` |
//! | translation sampler `output` (values **and** both `CUBICSPLINE` tangents) | unchanged | `* s` |
//! | scale sampler `output` (values **and** both `CUBICSPLINE` tangents) | `* 1/s` | unchanged |
//! | `inverseBindMatrices` rows 0..2 of the node's own slot | `* s` | `* s` |
//!
//! Inverse binds are the one domain whose multiplier is `s_i` rather than
//! `s_parent`, because `B_i' = C_i^-1 * B_i = scale(s_i) * B_i` — every
//! affected joint's slot scales, the root's included. Mesh `POSITION`,
//! normals, rotation tracks, key times, and scale tracks away from the
//! closure root are left byte-identical: the source's world geometry is
//! already correct and this operation only changes how it is parameterized.
//!
//! # The aliasing refusal this operation needs and #280 does not provide
//!
//! #280's `ConflictingAccessorUse` is a two-value classification
//! (scale-bearing vs dimensionless) and fires only on the cross. The
//! scale-bearing/scale-bearing cross is *accepted*, which is correct for a
//! whole-document conversion — every such use converts by the same `q` — and
//! fatal here, where the multiplier differs per node and per skin slot. All
//! four of these pass #280 today:
//!
//! - one translation `output` reached by channels targeting two different
//!   nodes, whose multipliers are `s_parent(A)` and `s_parent(B)`;
//! - one translation `output` shared by the closure root (multiplier `1`)
//!   and a child (multiplier `s`);
//! - one accessor used as mesh `POSITION` (which must stay byte-identical)
//!   and as a translation `output` (which scales by `s`);
//! - one `inverseBindMatrices` shared by two skins whose per-slot factors
//!   differ.
//!
//! #282 keyed its rewrite by unique accessor index and recorded that
//! copy-on-write was not the fix, because splitting a shared accessor changes
//! `accessors`/`bufferViews` lengths and destroys the array identities the
//! proof pins. That is still true. It also rested on type-disjointness, and
//! **that half no longer holds**: a `POSITION` and a translation `output` are
//! both `VEC3`-shaped, and two translation outputs are trivially the same
//! type.
//!
//! So this module builds an accessor-to-factor map covering every raw
//! accessor use this operation rewrites or promises to preserve. Factor-one
//! claims include every mesh attribute and index, morph attribute, sampler
//! input, rotation/weights output, unaffected scale output, and unreferenced
//! sampler output. That is exactly what makes the
//! `NORMAL`/root-scale-output and orphan-output crosses detectable. It
//! refuses same-index disagreement with
//! [`GltfScaleRewriteError::ConflictingRestBindFactor`], naming both
//! claimants and both factors so an operator can fix the file. Inside the raw
//! domain the common conservative preflight already admits, an accessor every
//! claimant agrees on adds no rest/bind-specific refusal: a `POSITION` shared
//! by two primitives, a time accessor shared by three samplers, and a
//! translation output shared by two nodes with the same multiplier all still
//! convert. This does not newly admit a scale-bearing/dimensionless alias the
//! common preflight already refuses. A second guard resolves all those raw
//! ranges and refuses distinct accessor indices whose bytes overlap when
//! either one is rewritten; same-index agreement alone cannot prevent that
//! corruption.
//!
//! # Absent `inverseBindMatrices`
//!
//! glTF's format default ("no `inverseBindMatrices`, every joint's bind is
//! identity") is unreachable here and would be unimplementable if it were
//! not: #280 refuses such a skin outright with `MissingInverseBinds`, and
//! materializing the array the way core's analytic reference fixture does
//! would append an accessor, a `bufferView` and buffer bytes — breaking
//! array identities, preserved-byte length equality, and
//! [`super::container`]'s no-length-change invariant at once. This module
//! therefore documents the case rather than mirroring core's materialization.

use super::bytes;
use super::plan::{
    GltfScalePlan, RawAccessorTarget, RestBindComponents, accessor_type, plan_mismatch,
};
use super::{GltfScaleArtifact, GltfScaleRewriteError};
use crate::LoadError;
use crate::capability::{
    GltfCapabilityViolation, GltfCapabilityViolationKind, GltfScaleSource, declared,
    resolved_accessor_range,
};
use animsmith_core::scale::{
    ScaleFieldDisposition, ScaleOperation, ScalePlan, ScaleRequest, ScaleRewriteRule,
    ScaleSourceRestField, plan_scale,
};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

// --- Accessor claims --------------------------------------------------------

fn writer_components_cover(components: RestBindComponents, component: usize) -> bool {
    !matches!(components, RestBindComponents::Mat4Rows) || component % 4 != 3
}

/// The per-element factors one claim demands.
#[derive(Debug, Clone, PartialEq)]
enum RestBindFactors {
    /// Every element scales by the same factor.
    Uniform(f64),
    /// One factor per element: an `inverseBindMatrices` accessor, whose slot
    /// `j` scales by `s` exactly when joint `j` is inside the closure.
    PerElement(Vec<f64>),
}

impl RestBindFactors {
    fn at(&self, element: usize) -> f64 {
        match self {
            Self::Uniform(factor) => *factor,
            Self::PerElement(factors) => factors.get(element).copied().unwrap_or(1.0),
        }
    }

    fn is_identity(&self, count: usize) -> bool {
        if count == 0 {
            return true;
        }
        match self {
            Self::Uniform(factor) => *factor == 1.0,
            Self::PerElement(factors) => factors.iter().take(count).all(|&factor| factor == 1.0),
        }
    }

    fn uniform_factor(&self, count: usize) -> Option<f64> {
        match self {
            Self::Uniform(factor) => Some(*factor),
            Self::PerElement(factors) => {
                let first = self.at(0);
                let explicit_agrees = factors
                    .iter()
                    .take(count)
                    .skip(1)
                    .all(|&factor| factor == first);
                let implicit_agrees = count <= factors.len() || first == 1.0;
                (explicit_agrees && implicit_agrees).then_some(first)
            }
        }
    }

    fn first_disagreement(&self, other: &Self, count: usize) -> Option<usize> {
        match (self, other) {
            (Self::Uniform(left), Self::Uniform(right)) => {
                (count != 0 && left != right).then_some(0)
            }
            (Self::Uniform(left), Self::PerElement(right)) => right
                .iter()
                .take(count)
                .position(|right| left != right)
                .or_else(|| (count > right.len() && *left != 1.0).then_some(right.len())),
            (Self::PerElement(left), Self::Uniform(right)) => left
                .iter()
                .take(count)
                .position(|left| left != right)
                .or_else(|| (count > left.len() && *right != 1.0).then_some(left.len())),
            (Self::PerElement(left), Self::PerElement(right)) => (0..count
                .min(left.len().max(right.len())))
                .find(|&element| self.at(element) != other.at(element)),
        }
    }
}

/// One accessor's demanded rest/bind rewrite, and the first location that
/// demanded it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RestBindClaim {
    components: RestBindComponents,
    factors: RestBindFactors,
    /// Element count, read from the accessor's own `count`.
    count: usize,
    /// JSON pointer of the first use that made this claim.
    location: String,
}

impl RestBindClaim {
    /// The multiplier component `component` of element `element` receives, or
    /// `None` when this claim leaves that value bit-identical.
    ///
    /// A factor of exactly `1.0` maps to `None` rather than to `Some(1.0)`
    /// because "an unaffected joint's inverse bind comes through
    /// byte-for-byte" is the claim this operation makes, and `Some(1.0)`
    /// routes the value through [`bytes::narrow`], which refuses a non-finite
    /// result.
    ///
    /// **On every input reachable today the two are equivalent**, and no test
    /// distinguishes them: `f64::from(x) * 1.0` narrows back to `x` bit for
    /// bit at every finite `x` including both signed zeros, and a non-finite
    /// stored value is refused by
    /// [`animsmith_core::scale::plan_scale`]'s document-shape validation
    /// before this function is called. Replacing this with `Some(...)`
    /// unconditionally is an equivalent mutation; it is written this way so
    /// the preservation claim does not depend on that validation staying
    /// exactly where it is.
    pub(crate) fn multiplier(&self, element: usize, component: usize) -> Option<f64> {
        if !writer_components_cover(self.components, component) {
            return None;
        }
        let factor = self.factors.at(element);
        (factor != 1.0).then_some(factor)
    }

    /// Whether this claim changes nothing, so the accessor is not rewritten
    /// at all.
    pub(crate) fn is_identity(&self) -> bool {
        self.factors.is_identity(self.count)
    }

    /// The single factor every element of this claim shares, when there is
    /// one.
    ///
    /// Authored `min`/`max` can only be converted by a single factor; a claim
    /// with mixed per-slot factors has no such conversion and its bounds are
    /// re-derived from the rewritten payload instead.
    pub(crate) fn uniform_factor(&self) -> Option<f64> {
        self.factors.uniform_factor(self.count)
    }

    /// The first element at which two claims on one accessor disagree.
    fn first_disagreement(&self, other: &Self) -> Option<usize> {
        if self.components != other.components {
            // Two masks over the same declared type are equivalent when both
            // uses preserve every value. This matters for an unaffected IBM
            // shared with another MAT4 role: no write is required, so the
            // narrower IBM row mask is not a real disagreement.
            return (!self.is_identity() || !other.is_identity()).then_some(0);
        }
        self.factors
            .first_disagreement(&other.factors, self.count.max(other.count))
    }
}

fn rest_bind_factor(
    disposition: ScaleFieldDisposition,
    factor: f64,
) -> Result<f64, GltfScaleRewriteError> {
    match disposition {
        ScaleFieldDisposition::PreserveExact => Ok(1.0),
        ScaleFieldDisposition::Rewrite(ScaleRewriteRule::RestBindParentBasis) => Ok(factor),
        ScaleFieldDisposition::Rewrite(ScaleRewriteRule::RestBindLocalScale) => Ok(1.0 / factor),
        _ => Err(plan_mismatch("invalid_rest_bind_field_disposition")),
    }
}

fn collect_rest_bind_claims(
    plan: &GltfScalePlan,
    factor: f64,
) -> Result<BTreeMap<usize, RestBindClaim>, GltfScaleRewriteError> {
    let mut claims: BTreeMap<usize, RestBindClaim> = BTreeMap::new();
    let claim = |accessor_index: usize,
                 new: RestBindClaim,
                 claims: &mut BTreeMap<usize, RestBindClaim>|
     -> Result<(), GltfScaleRewriteError> {
        match claims.get(&accessor_index) {
            Some(existing) => {
                if let Some(element) = existing.first_disagreement(&new) {
                    return Err(GltfScaleRewriteError::ConflictingRestBindFactor {
                        accessor_index,
                        element,
                        first_location: existing.location.clone(),
                        first_factor: existing.factors.at(element),
                        second_location: new.location,
                        second_factor: new.factors.at(element),
                    });
                }
            }
            None => {
                claims.insert(accessor_index, new);
            }
        }
        Ok(())
    };

    for binding in plan.accessor_bindings() {
        let factors = match &binding.target {
            RawAccessorTarget::PreserveExact => RestBindFactors::Uniform(1.0),
            RawAccessorTarget::MeshNormals { .. } | RawAccessorTarget::MeshPositions { .. } => {
                RestBindFactors::Uniform(1.0)
            }
            RawAccessorTarget::InstanceInverseBind { source_skin_index } => {
                let skin = plan.skin_binding(*source_skin_index)?;
                RestBindFactors::PerElement(
                    skin.slots
                        .iter()
                        .map(|slot| {
                            let joint = slot.source_node_index;
                            if plan.is_rest_bind_affected(joint) {
                                factor
                            } else {
                                1.0
                            }
                        })
                        .collect(),
                )
            }
            RawAccessorTarget::Animation {
                disposition,
                property: _,
                source_node_index: _,
            } => RestBindFactors::Uniform(rest_bind_factor(*disposition, factor)?),
        };
        claim(
            binding.accessor_index,
            RestBindClaim {
                components: binding.components,
                factors,
                count: binding.count,
                location: binding.location.clone(),
            },
            &mut claims,
        )?;
    }
    Ok(claims)
}

/// Refuse distinct raw accessor identities whose byte ranges overlap when at
/// least one is rewritten by this operation.
fn reject_rest_bind_accessor_overlaps(
    root: &Map<String, Value>,
    buffers: &[Vec<u8>],
    claims: &BTreeMap<usize, RestBindClaim>,
) -> Result<(), GltfScaleRewriteError> {
    let mut ranges = Vec::with_capacity(claims.len());
    for (&accessor_index, claim) in claims {
        let (buffer, start, end) = resolved_accessor_range(root, buffers, accessor_index)
            .ok_or_else(|| GltfScaleRewriteError::UnrewritableAccessor {
                accessor_index,
                location: format!("/accessors/{accessor_index}"),
            })?;
        if start < end {
            ranges.push((buffer, start, end, accessor_index, !claim.is_identity()));
        }
    }
    let Some((left, right)) = first_rewrite_overlap(&mut ranges) else {
        return Ok(());
    };
    let violations = BTreeSet::from([left, right])
        .into_iter()
        .map(|accessor_index| GltfCapabilityViolation {
            kind: GltfCapabilityViolationKind::OverlappingAccessorRanges,
            location: format!("/accessors/{accessor_index}"),
        })
        .collect::<Vec<_>>();
    let count = violations.len();
    Err(GltfScaleRewriteError::Capability { violations, count })
}

/// First deterministic conflicting pair in `O(n log n)` time.
fn first_rewrite_overlap(
    ranges: &mut [(usize, usize, usize, usize, bool)],
) -> Option<(usize, usize)> {
    ranges.sort_unstable();
    let mut active_buffer = None;
    let mut prior_any: Option<(usize, usize)> = None;
    let mut prior_rewritten: Option<(usize, usize)> = None;
    for &(buffer, start, end, accessor_index, rewritten) in ranges.iter() {
        if active_buffer != Some(buffer) {
            active_buffer = Some(buffer);
            prior_any = None;
            prior_rewritten = None;
        }
        let prior = if rewritten {
            prior_any
        } else {
            prior_rewritten
        };
        if let Some((prior_end, prior_index)) = prior
            && start < prior_end
        {
            return Some((prior_index, accessor_index));
        }
        if prior_any.is_none_or(|(prior_end, _)| end > prior_end) {
            prior_any = Some((end, accessor_index));
        }
        if rewritten && prior_rewritten.is_none_or(|(prior_end, _)| end > prior_end) {
            prior_rewritten = Some((end, accessor_index));
        }
    }
    None
}

// --- Node JSON rewrites -----------------------------------------------------

/// How one raw node transform member is rebased.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum NodeRebase {
    /// Node `translation`: every entry by `s_parent`.
    Translation(f64),
    /// Node `scale`: every entry by `s_parent / s_i`, materializing
    /// `[factor; 3]` when the member is absent, since glTF's default is
    /// `[1, 1, 1]` and `1 * factor` is the value the output must declare.
    Scale(f64),
    /// Column-major node `matrix`: the nine linear entries by `s_parent / s_i`
    /// and the three translation entries by `s_parent`. Components `3, 7, 11,
    /// 15` are the homogeneous row, which #280 has already proved is exactly
    /// `(0, 0, 0, 1)`, so it is left untouched rather than multiplied.
    Matrix {
        /// Multiplier for components `0,1,2, 4,5,6, 8,9,10`.
        linear: f64,
        /// Multiplier for components `12, 13, 14`.
        translation: f64,
    },
}

impl NodeRebase {
    pub(crate) fn member(self) -> &'static str {
        match self {
            Self::Translation(_) => "translation",
            Self::Scale(_) => "scale",
            Self::Matrix { .. } => "matrix",
        }
    }

    pub(crate) fn expected_len(self) -> usize {
        match self {
            Self::Translation(_) | Self::Scale(_) => 3,
            Self::Matrix { .. } => 16,
        }
    }

    /// The multiplier entry `component` receives, or `None` when the entry is
    /// left exactly as authored.
    pub(crate) fn multiplier(self, component: usize) -> Option<f64> {
        let factor = match self {
            Self::Translation(factor) | Self::Scale(factor) => Some(factor),
            Self::Matrix {
                linear,
                translation,
            } => match component {
                0..=2 | 4..=6 | 8..=10 => Some(linear),
                12..=14 => Some(translation),
                _ => None,
            },
        };
        factor.filter(|&factor| factor != 1.0)
    }
}

/// Every node transform member this plan rebases, in source node order.
///
/// A member whose multiplier is exactly `1.0` is not emitted, which is what
/// makes a declared factor of one a deterministic no-op rather than a
/// re-render of every affected node's transform.
fn collect_plan_node_rebases(
    plan: &GltfScalePlan,
    factor: f64,
) -> Result<Vec<(usize, NodeRebase)>, GltfScaleRewriteError> {
    let mut out = Vec::new();
    for node in plan.node_bindings().iter().filter(|node| {
        matches!(
            node.kind,
            animsmith_core::scale::ScaleSourceNodeKind::Projected { .. }
                | animsmith_core::scale::ScaleSourceNodeKind::Connector
        )
    }) {
        let node_index = node.source_node_index;
        if node.matrix_declared {
            let linear = rest_bind_source_factor(
                plan.source_rest(node_index, ScaleSourceRestField::MatrixLinear)?,
                factor,
                ScaleSourceRestField::MatrixLinear,
            )?;
            let translation = rest_bind_source_factor(
                plan.source_rest(node_index, ScaleSourceRestField::MatrixTranslation)?,
                factor,
                ScaleSourceRestField::MatrixTranslation,
            )?;
            if linear != 1.0 || translation != 1.0 {
                out.push((
                    node_index,
                    NodeRebase::Matrix {
                        linear,
                        translation,
                    },
                ));
            }
            continue;
        }
        let translation = rest_bind_source_factor(
            plan.source_rest(node_index, ScaleSourceRestField::Translation)?,
            factor,
            ScaleSourceRestField::Translation,
        )?;
        if translation != 1.0 && node.translation_declared {
            out.push((node_index, NodeRebase::Translation(translation)));
        }
        let scale = rest_bind_source_factor(
            plan.source_rest(node_index, ScaleSourceRestField::Scale)?,
            factor,
            ScaleSourceRestField::Scale,
        )?;
        if scale != 1.0 {
            out.push((node_index, NodeRebase::Scale(scale)));
        }
    }
    Ok(out)
}

fn rest_bind_source_factor(
    disposition: ScaleFieldDisposition,
    factor: f64,
    field: ScaleSourceRestField,
) -> Result<f64, GltfScaleRewriteError> {
    match disposition {
        ScaleFieldDisposition::PreserveExact => Ok(1.0),
        ScaleFieldDisposition::Rewrite(ScaleRewriteRule::RestBindSourceLocal {
            connector_tail: None,
        }) => match field {
            ScaleSourceRestField::Translation | ScaleSourceRestField::MatrixTranslation => {
                Ok(factor)
            }
            ScaleSourceRestField::Scale | ScaleSourceRestField::MatrixLinear => Ok(1.0 / factor),
            _ => Err(plan_mismatch("invalid_rest_bind_source_field")),
        },
        ScaleFieldDisposition::Rewrite(ScaleRewriteRule::RestBindSourceLocal {
            connector_tail: Some(_),
        }) => Err(plan_mismatch("gltf_connector_source_rewrite_unsupported")),
        _ => Err(plan_mismatch("invalid_rest_bind_source_field_disposition")),
    }
}

// --- Rewrite ----------------------------------------------------------------

/// Reparameterize `source`'s rest/bind hierarchy, removing the compensating
/// uniform factor `expected_factor` from the closure anchored at
/// `source_root_node_index` and the joints of `source_skin_index`.
///
/// Both selectors are required raw source identity — a `nodes` array index
/// and a `skins` array index — and the factor is declared, never inferred.
/// A declared factor of one is a deterministic no-op: every multiplier is
/// exactly `1.0`, so no accessor and no node member is selected and the
/// artifact's JSON tree and buffer bytes are value-identical to the source's.
/// Any observed mismatch between the declared factor and the source's
/// measured rest-world factor rejects through
/// [`animsmith_core::scale::plan_scale`].
///
/// # Errors
///
/// Returns [`GltfScaleRewriteError::Capability`] for a manifest declaring an
/// unpreservable domain, [`GltfScaleRewriteError::Plan`] for every shared
/// planning rejection — invalid or unrepresentable factor, invalid selector,
/// incomplete closure, unskinned geometry in the closure, non-uniform,
/// sheared, reflected, singular or non-finite affine, factor mismatch, mixed
/// factor, malformed track or instance data —
/// [`GltfScaleRewriteError::ConflictingRestBindFactor`] when two logical uses
/// of one accessor demand different factors,
/// [`GltfScaleRewriteError::ClosureMismatch`] or
/// [`GltfScaleRewriteError::ParentChainDisagreement`] when the raw hierarchy,
/// the plan's closure and the normalized skeleton do not describe the same
/// tree, [`GltfScaleRewriteError::AmbiguousSourceNodeProjection`] when two
/// source nodes claim one bone,
/// [`GltfScaleRewriteError::UnusableSourceHierarchy`] for a raw hierarchy the
/// closure cannot be derived from,
/// [`GltfScaleRewriteError::ConflictingNodeTransform`] and
/// [`GltfScaleRewriteError::NonAffineNodeMatrix`] for an out-of-contract node
/// transform, [`GltfScaleRewriteError::UnrewritableAccessor`] for an accessor
/// outside the dense `f32` layout,
/// [`GltfScaleRewriteError::ImagePayloadOverlap`] when an image payload
/// shares bytes with a rewritten accessor,
/// [`GltfScaleRewriteError::ValueNotRepresentable`] for an element whose
/// rebased value has no `f32` image, and
/// [`GltfScaleRewriteError::Write`] when a GLB length field would overflow.
pub fn rewrite_rest_bind(
    source: &GltfScaleSource,
    source_skin_index: usize,
    source_root_node_index: usize,
    expected_factor: f64,
) -> Result<GltfScaleArtifact, GltfScaleRewriteError> {
    let facts = super::require_scale_capability(source.manifest())?;
    let operation = ScaleOperation::RestBindUniformScale {
        source_skin_index,
        source_root_node_index,
        expected_factor,
    };
    let plan = plan_scale(&ScaleRequest {
        operation,
        document: source.document(),
        capability: &facts,
    })?;

    rewrite_rest_bind_plan(source, &plan)
}

pub(crate) fn rewrite_rest_bind_plan(
    source: &GltfScaleSource,
    plan: &ScalePlan,
) -> Result<GltfScaleArtifact, GltfScaleRewriteError> {
    let manifest = source.manifest();
    let ScaleOperation::RestBindUniformScale {
        source_skin_index: _,
        source_root_node_index: _,
        expected_factor,
    } = plan.operation()
    else {
        return Err(plan_mismatch("gltf_operation_plan_mismatch"));
    };
    let gltf_plan = GltfScalePlan::new(source, plan)?;

    let root = source
        .raw_json()
        .as_object()
        .ok_or_else(|| LoadError::Malformed("top-level glTF JSON is not an object".into()))?;
    super::reject_out_of_contract_nodes(root)?;

    let claims = collect_rest_bind_claims(&gltf_plan, expected_factor)?;
    reject_rest_bind_accessor_overlaps(root, source.resolved_buffers(), &claims)?;
    let rewritten: BTreeMap<usize, RestBindClaim> = claims
        .into_iter()
        .filter(|(_, claim)| !claim.is_identity())
        .collect();

    let mut spans = Vec::with_capacity(rewritten.len());
    for (&accessor_index, claim) in &rewritten {
        // `accessor_span_typed` resolves the dense `f32` range and checks the
        // accessor `type` the claim requires. It also asserts that the range
        // holds exactly `count * components` floats — reading `count` from
        // this same `/accessors/{i}/count`, which is where `RestBindClaim`'s
        // own `count` came from. A second check of `span.float_count()`
        // against `claim.count * span.components` here would compare that
        // assertion with itself, so there is not one.
        spans.push((
            bytes::accessor_span_typed(
                root,
                source.resolved_buffers(),
                accessor_index,
                Some(accessor_type(claim.components)),
            )?,
            claim,
        ));
    }
    super::reject_image_payload_overlap_spans(root, manifest, spans.iter().map(|(span, _)| *span))?;

    let mut buffers = source.resolved_buffers().to_vec();
    let mut extrema = BTreeMap::new();
    let mut modified = BTreeSet::new();
    for (span, claim) in &spans {
        extrema.insert(
            span.accessor_index,
            bytes::scale_span_with(&mut buffers, *span, &|element, component| {
                claim.multiplier(element, component)
            })?,
        );
        modified.insert(span.buffer);
    }

    let mut json = source.raw_json().clone();
    let mut rewritten_json_pointers = Vec::new();
    for (node_index, rebase) in collect_plan_node_rebases(&gltf_plan, expected_factor)? {
        rewrite_node_member(&mut json, node_index, rebase)?;
        rewritten_json_pointers.push(format!("/nodes/{node_index}/{}", rebase.member()));
    }
    for (&accessor_index, claim) in &rewritten {
        rewritten_json_pointers.extend(super::rewrite_accessor_bounds_with(
            &mut json,
            accessor_index,
            &|component| writer_components_cover(claim.components, component),
            claim.uniform_factor(),
            &extrema[&accessor_index],
        )?);
    }
    rewritten_json_pointers.sort();

    let reencoded_buffers = modified
        .iter()
        .copied()
        .filter(|&buffer_index| {
            manifest.buffers.get(buffer_index).is_some_and(|buffer| {
                buffer.source_kind == crate::capability::GltfBufferSourceKind::DataUri
            })
        })
        .collect();
    let affected_source_skins = affected_skins_from_plan(&gltf_plan);
    let out = super::container::assemble(manifest, &json, &buffers, &modified)?;
    Ok(GltfScaleArtifact {
        container: manifest.container,
        bytes: out,
        rewritten_accessors: rewritten.keys().copied().collect(),
        rewritten_json_pointers,
        reencoded_buffers,
        // The raw closure this rewrite was driven by, in the same source-node
        // index space `source_root_node_index` selected it with.
        affected_source_nodes: gltf_plan.affected_source_nodes(true),
        affected_source_skins,
        declared_factor: expected_factor,
        operation: plan.operation(),
    })
}

fn affected_skins_from_plan(plan: &GltfScalePlan) -> Vec<usize> {
    plan.skin_bindings()
        .iter()
        .filter(|skin| {
            skin.slots
                .iter()
                .any(|slot| plan.is_rest_bind_affected(slot.source_node_index))
        })
        .map(|skin| skin.source_skin_index)
        .collect()
}

/// Rebase one node transform member in place, materializing an absent
/// `scale`.
fn rewrite_node_member(
    json: &mut Value,
    node_index: usize,
    rebase: NodeRebase,
) -> Result<(), GltfScaleRewriteError> {
    let pointer = format!("/nodes/{node_index}/{}", rebase.member());
    let node_pointer = format!("/nodes/{node_index}");
    let is_declared = json
        .pointer(&node_pointer)
        .is_some_and(|node| declared(node, rebase.member()).is_some());
    if !is_declared {
        // Only `scale` has a non-zero glTF default, so only `scale` needs
        // materializing: an absent `translation` defaults to `(0, 0, 0)`,
        // which is fixed under multiplication, and an absent `matrix`
        // declares no matrix at all. Materializing the member adds no array
        // element and shifts no index, so every array identity the artifact
        // proof pins survives it.
        let NodeRebase::Scale(factor) = rebase else {
            return Ok(());
        };
        let value = number(bytes::narrow(factor, &pointer)?, &pointer)?;
        json.pointer_mut(&node_pointer)
            .and_then(Value::as_object_mut)
            .ok_or_else(|| LoadError::Malformed(format!("{node_pointer} is not an object")))?
            .insert(
                "scale".to_owned(),
                Value::Array(vec![value.clone(), value.clone(), value]),
            );
        return Ok(());
    }
    let entries = json
        .pointer_mut(&pointer)
        .and_then(Value::as_array_mut)
        .filter(|entries| entries.len() == rebase.expected_len())
        .ok_or_else(|| {
            LoadError::Malformed(format!(
                "{pointer} is not an array of {} numbers",
                rebase.expected_len()
            ))
        })?;
    for (component, entry) in entries.iter_mut().enumerate() {
        let Some(multiplier) = rebase.multiplier(component) else {
            continue;
        };
        let location = format!("{pointer}/{component}");
        let before = entry
            .as_f64()
            .ok_or_else(|| LoadError::Malformed(format!("{location} is not a number")))?;
        *entry = number(bytes::narrow(before * multiplier, &location)?, &location)?;
    }
    Ok(())
}

fn number(value: f32, location: &str) -> Result<Value, GltfScaleRewriteError> {
    super::number(value, location)
}

#[cfg(test)]
mod tests {
    //! Unit tests for the structural checks `rewrite_rest_bind` performs
    //! before it writes a byte.
    //!
    //! The agreement checks cannot be falsified by any glTF byte sequence.
    //! The raw child arrays, `SourceNodeAsset::parent_source_node_index` and
    //! `Skeleton::parent` are all derived by this crate's own `topology` from
    //! one parsed document, so no source makes them disagree — which is
    //! exactly why these are classification tests over a mutated `Document`
    //! rather than end-to-end ones. What they pin is that each disagreement
    //! is *named*, and — the direction that has caught more in this lane —
    //! that the unmutated document still passes the canonical adapter.
    //!
    //! Every mutation goes to the `Document` handed straight to the checks,
    //! never through a re-plan, because `animsmith_core`'s own
    //! chain-agreement validation refuses most of these shapes first: under
    //! `Complete` coverage a projection that contradicts its skeleton, or one
    //! that sends two source nodes to a single bone, never survives
    //! `plan_scale`. So a mutation that needs the skeleton and the projection
    //! to disagree with the *raw children* moves those two together, leaving
    //! the raw child arrays — the one description `animsmith_core` cannot
    //! see — as the sole dissenter; and a mutation core would itself refuse
    //! pins nothing about reachability, only that the adapter still names the
    //! compatibility error while it binds the raw hierarchy to the compiled
    //! plan. Their *wiring* is pinned through
    //! `capability::scale_source_with_document`: the seam hands the rewriter a
    //! real source whose normalized projection contradicts its own bytes,
    //! which is the one relaxation gate bypass cannot supply.
    //!
    use super::*;
    use crate::preflight_scale_source_bytes;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use serde_json::json;
    use std::path::Path;

    /// The §D.3 case 2 hierarchy, with the smallest payload that preflights:
    /// `root(scale 0.01) -> joint -> attach`, plus a skinned holder outside
    /// the closure.
    fn source() -> GltfScaleSource {
        // 36 POSITION | 24 JOINTS | 48 WEIGHTS | 64 inverse bind = 172 bytes.
        let mut buffer = vec![0u8; 172];
        let mut put = |offset: usize, values: &[f32]| {
            for (index, value) in values.iter().enumerate() {
                let at = offset + index * 4;
                buffer[at..at + 4].copy_from_slice(&value.to_le_bytes());
            }
        };
        put(0, &[0.0, 1.0, 0.0, 0.5, 1.25, -0.25, -0.5, 0.75, 0.5]);
        put(
            60,
            &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
        );
        put(
            108,
            &[
                100.0, 0.0, 0.0, 0.0, //
                0.0, 100.0, 0.0, 0.0, //
                0.0, 0.0, 100.0, 0.0, //
                0.0, -100.0, 0.0, 1.0,
            ],
        );
        let value = json!({
            "asset": { "version": "2.0" },
            "buffers": [{
                "uri": format!(
                    "data:application/octet-stream;base64,{}",
                    STANDARD.encode(&buffer)
                ),
                "byteLength": 172
            }],
            "bufferViews": [
                { "buffer": 0, "byteOffset": 0, "byteLength": 36 },
                { "buffer": 0, "byteOffset": 36, "byteLength": 24 },
                { "buffer": 0, "byteOffset": 60, "byteLength": 48 },
                { "buffer": 0, "byteOffset": 108, "byteLength": 64 }
            ],
            "accessors": [
                { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                  "min": [-0.5, 0.75, -0.25], "max": [0.5, 1.25, 0.5] },
                { "bufferView": 1, "componentType": 5123, "count": 3, "type": "VEC4" },
                { "bufferView": 2, "componentType": 5126, "count": 3, "type": "VEC4" },
                { "bufferView": 3, "componentType": 5126, "count": 1, "type": "MAT4" }
            ],
            "meshes": [{ "primitives": [{
                "attributes": { "POSITION": 0, "JOINTS_0": 1, "WEIGHTS_0": 2 }
            }] }],
            "nodes": [
                { "name": "root", "scale": [0.01, 0.01, 0.01], "children": [1] },
                { "name": "joint", "translation": [0.0, 100.0, 0.0], "children": [2] },
                { "name": "attach", "translation": [1.0, 0.0, 0.0] },
                { "name": "holder", "mesh": 0, "skin": 0 }
            ],
            "scenes": [{ "nodes": [0, 3] }],
            "scene": 0,
            "skins": [{ "joints": [1], "skeleton": 0, "inverseBindMatrices": 3 }]
        });
        preflight_scale_source_bytes(
            Path::new("rest-bind-unit.gltf"),
            &serde_json::to_vec(&value).expect("fixture serializes"),
        )
        .expect("the fixture preflights cleanly")
    }

    fn plan(source: &GltfScaleSource) -> ScalePlan {
        plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: source.document(),
            capability: &super::super::capability_facts(source.manifest()),
        })
        .expect("plan")
    }

    // --- The shared #282 guards are still wired into this rewrite ---------
    //
    // #280's preflight refuses both out-of-contract node transforms (#301)
    // and image payloads aliasing a scale-bearing accessor (#300), so no
    // `GltfScaleSource` carrying either can be built through the public API.
    // The guards are kept — they are the layer that must hold if the gate is
    // relaxed — and `capability::scale_source_past_the_gate` is the
    // `cfg(test)`-only seam that supplies exactly that relaxation. Without
    // these two, deleting either call site from `rewrite_rest_bind` changes
    // no observable behaviour and no test fails.

    /// The unit fixture's JSON, as a mutable tree.
    fn fixture_value() -> Value {
        serde_json::from_slice(source().source_bytes()).expect("the fixture is JSON")
    }

    fn past_the_gate(name: &str, value: &Value) -> GltfScaleSource {
        let bytes = serde_json::to_vec(value).expect("fixture serializes");
        crate::capability::scale_source_past_the_gate(Path::new(name), &bytes)
            .unwrap_or_else(|error| panic!("{name} must load past the gate: {error:?}"))
    }

    #[test]
    fn rewrite_rest_bind_still_calls_the_node_transform_guard() {
        let mut value = fixture_value();
        value["nodes"][3]["matrix"] = json!([
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0
        ]);
        value["nodes"][3]["translation"] = json!([1.5, -2.0, 0.25]);
        let source = past_the_gate("matrix-plus-trs.gltf", &value);
        match rewrite_rest_bind(&source, 0, 0, 0.01) {
            Err(GltfScaleRewriteError::ConflictingNodeTransform { location }) => {
                assert_eq!(location, "/nodes/3/translation");
            }
            other => panic!("the wired guard must refuse matrix + translation, got {other:?}"),
        }
    }

    #[test]
    fn rewrite_rest_bind_still_calls_the_image_payload_guard() {
        // An image reading the inverse-bind range, which this operation
        // rewrites. Without the guard the rebase runs to completion and
        // writes rebased `f32`s over bytes the image reads.
        let mut value = fixture_value();
        value["bufferViews"]
            .as_array_mut()
            .expect("bufferViews")
            .push(json!({ "buffer": 0, "byteOffset": 120, "byteLength": 16 }));
        value["images"] = json!([{ "bufferView": 4, "mimeType": "image/png" }]);
        let source = past_the_gate("image-overlap.gltf", &value);
        match rewrite_rest_bind(&source, 0, 0, 0.01) {
            Err(GltfScaleRewriteError::ImagePayloadOverlap {
                location,
                accessor_index,
            }) => {
                assert_eq!(location, "/images/0/bufferView");
                assert_eq!(accessor_index, 3);
            }
            other => panic!("the wired guard must refuse an aliased image, got {other:?}"),
        }
    }

    #[test]
    fn the_wired_image_guard_still_accepts_a_disjoint_image_view() {
        // The seam must not make every source refusable: the same document
        // with the image moved clear of every rewritten accessor rebases.
        // This is what keeps the two tests above from passing for the wrong
        // reason.
        let mut value = fixture_value();
        value["bufferViews"]
            .as_array_mut()
            .expect("bufferViews")
            .push(json!({ "buffer": 0, "byteOffset": 36, "byteLength": 16 }));
        value["images"] = json!([{ "bufferView": 4, "mimeType": "image/png" }]);
        let source = past_the_gate("image-disjoint.gltf", &value);
        rewrite_rest_bind(&source, 0, 0, 0.01)
            .expect("an image view disjoint from every rewritten span rebases");
    }

    #[test]
    fn compatibility_precheck_names_an_ambiguous_source_projection() {
        let source = source();
        let plan = plan(&source);
        let mut document = source.document().clone();
        document.assets.source_skeleton.nodes[2].bone = Some(1);
        let ambiguous = crate::capability::scale_source_with_document(source, document);

        match super::super::plan::GltfScalePlan::new(&ambiguous, &plan) {
            Err(GltfScaleRewriteError::AmbiguousSourceNodeProjection { bone }) => {
                assert_eq!(bone, 1);
            }
            Ok(_) => panic!("expected AmbiguousSourceNodeProjection, got success"),
            Err(error) => panic!("expected AmbiguousSourceNodeProjection, got {error:?}"),
        }
    }

    #[test]
    fn raw_source_skin_identity_is_cross_checked_through_the_plan_adapter() {
        let cases = [
            ("raw_source_skin_joint_mismatch", 0usize),
            ("raw_source_skin_identity_mismatch", 1usize),
            ("raw_source_skin_count_mismatch", 2usize),
        ];

        for (expected_reason, case) in cases {
            let source = source();
            let mut document = source.document().clone();
            match case {
                0 => {
                    document.assets.source_skeleton.skins[0].joint_source_node_indices = vec![2];
                }
                1 => {
                    document.assets.source_skeleton.skins[0].skeleton_root_source_node_index =
                        Some(1);
                }
                2 => {
                    document.assets.source_skeleton.skins.push(
                        animsmith_core::model::SourceSkinAsset {
                            source_skin_index: 1,
                            ..Default::default()
                        },
                    );
                }
                _ => unreachable!(),
            }
            let plan = plan_scale(&ScaleRequest {
                operation: ScaleOperation::WholeDocumentLinearUnits { factor: 2.0 },
                document: &document,
                capability: &super::super::capability_facts(source.manifest()),
            })
            .expect("the doctored source inventory still plans");
            let retargeted = crate::capability::scale_source_with_document(source, document);

            assert!(matches!(
                super::super::plan::GltfScalePlan::new(&retargeted, &plan),
                Err(GltfScaleRewriteError::Plan(
                    animsmith_core::scale::ScaleError::PlanDocumentMismatch { reason }
                )) if reason == expected_reason
            ));
        }
    }

    #[test]
    fn rewrite_rest_bind_keeps_the_closure_mismatch_error_classification() {
        let source = source();
        let mut document = source.document().clone();
        document.assets.source_skeleton.nodes[2].parent_source_node_index = None;
        document.skeleton.bones[2].parent = None;
        let plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &document,
            capability: &super::super::capability_facts(source.manifest()),
        })
        .expect("a smaller canonical closure still plans");
        let contradicting = crate::capability::scale_source_with_document(source, document);

        match rewrite_rest_bind_plan(&contradicting, &plan) {
            Err(GltfScaleRewriteError::ClosureMismatch { planned, derived }) => {
                assert_eq!(planned, vec![0, 1]);
                assert_eq!(derived, vec![0, 1, 2]);
            }
            other => panic!("expected ClosureMismatch, got {other:?}"),
        }
    }

    #[test]
    fn rewrite_rest_bind_cross_checks_raw_children_with_plan_topology() {
        // Deleting the numeric-free plan adapter from `rewrite_rest_bind`
        // changes no ordinary fixture output without this mutation.
        // `capability::scale_source_with_document` is the `cfg(test)`-only
        // seam that supplies the relaxation no glTF byte sequence can — a
        // source whose normalized skeleton contradicts its own raw children.
        let source = source();
        let mut document = source.document().clone();
        // Bone 2 is source node 2, whose raw parent is node 1 (bone 1).
        // Skeleton *and* projection are moved together onto node 0, so
        // `animsmith_core`'s chain-agreement validation is satisfied and the
        // raw child arrays are the only description left disagreeing — which
        // is the disagreement only this layer can see.
        document.skeleton.bones[2].parent = Some(0);
        document.assets.source_skeleton.nodes[2].parent_source_node_index = Some(0);
        let contradicting = crate::capability::scale_source_with_document(source, document);
        match rewrite_rest_bind(&contradicting, 0, 0, 0.01) {
            Err(GltfScaleRewriteError::ParentChainDisagreement { source_node_index }) => {
                assert_eq!(source_node_index, 2);
            }
            other => {
                panic!(
                    "the plan topology check must refuse a contradicting skeleton, got {other:?}"
                )
            }
        }
    }

    #[test]
    fn the_plan_topology_check_accepts_an_untouched_document() {
        // The seam must not make every source refusable: the same source with
        // its own document handed back rebases. This is what keeps the test
        // above from passing for the wrong reason.
        let source = source();
        let document = source.document().clone();
        let unchanged = crate::capability::scale_source_with_document(source, document);
        rewrite_rest_bind(&unchanged, 0, 0, 0.01)
            .expect("a source whose document is its own still rebases");
    }

    #[test]
    fn the_mat4_row_mask_covers_exactly_the_three_output_rows() {
        let covered: Vec<usize> = (0..16)
            .filter(|&component| writer_components_cover(RestBindComponents::Mat4Rows, component))
            .collect();
        assert_eq!(covered, vec![0, 1, 2, 4, 5, 6, 8, 9, 10, 12, 13, 14]);
        for components in [
            RestBindComponents::Scalar,
            RestBindComponents::Vec2,
            RestBindComponents::Vec3,
            RestBindComponents::Vec4,
            RestBindComponents::Mat2,
            RestBindComponents::Mat3,
            RestBindComponents::Mat4,
        ] {
            assert!((0..16).all(|component| writer_components_cover(components, component)));
        }
    }

    #[test]
    fn overlap_sweep_covers_both_role_orders_nested_spans_and_skips_empty_ranges() {
        type Range = (usize, usize, usize, usize, bool);
        type Case = (&'static str, Vec<Range>, Option<(usize, usize)>);
        let cases: [Case; 4] = [
            (
                "preserved before rewritten",
                vec![(0, 0, 20, 1, false), (0, 5, 10, 2, true)],
                Some((1, 2)),
            ),
            (
                "rewritten before preserved",
                vec![(0, 0, 20, 1, true), (0, 5, 10, 2, false)],
                Some((1, 2)),
            ),
            (
                "a long preserved span still reaches a later rewrite",
                vec![
                    (0, 0, 200, 1, false),
                    (0, 50, 60, 2, false),
                    (0, 70, 80, 3, true),
                ],
                Some((1, 3)),
            ),
            (
                "empty and adjacent spans are disjoint",
                vec![
                    (0, 0, 10, 1, true),
                    (0, 10, 10, 2, false),
                    (0, 10, 20, 3, false),
                ],
                None,
            ),
        ];
        for (name, mut ranges, expected) in cases {
            ranges.retain(|(_, start, end, _, _)| start < end);
            assert_eq!(first_rewrite_overlap(&mut ranges), expected, "{name}");
        }
    }

    #[test]
    fn uniform_rewriter_factors_are_constant_time_for_huge_counts() {
        let identity = RestBindClaim {
            components: RestBindComponents::Vec3,
            factors: RestBindFactors::Uniform(1.0),
            count: usize::MAX,
            location: "/huge/identity".to_owned(),
        };
        let scaled = RestBindClaim {
            components: RestBindComponents::Vec3,
            factors: RestBindFactors::Uniform(100.0),
            count: usize::MAX,
            location: "/huge/scaled".to_owned(),
        };
        assert!(identity.is_identity());
        assert_eq!(identity.uniform_factor(), Some(1.0));
        assert_eq!(scaled.uniform_factor(), Some(100.0));
        assert_eq!(scaled.first_disagreement(&identity), Some(0));
        let mut empty_scaled = scaled.clone();
        empty_scaled.count = 0;
        let mut empty_identity = identity.clone();
        empty_identity.count = 0;
        assert!(empty_scaled.is_identity());
        assert_eq!(empty_scaled.first_disagreement(&empty_identity), None);
    }
}
