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
//! | `inverseBindMatrices` rows 0..2 of the node's own slot | `* s` | `* s` |
//!
//! Inverse binds are the one domain whose multiplier is `s_i` rather than
//! `s_parent`, because `B_i' = C_i^-1 * B_i = scale(s_i) * B_i` — every
//! affected joint's slot scales, the root's included. Mesh `POSITION`,
//! normals, rotation tracks, key times, and every scale/rotation sampler are
//! left byte-identical: the source's world geometry is already correct and
//! this operation only changes how it is parameterized.
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
//! So this module builds an accessor-to-factor map covering *every*
//! scale-bearing accessor — including the ones whose factor is `1`, which is
//! exactly what makes the `POSITION`/translation-output cross detectable —
//! and refuses on disagreement with
//! [`GltfScaleRewriteError::ConflictingRestBindFactor`], naming both
//! claimants and both factors so an operator can fix the file. An accessor
//! every claimant agrees on is fine however many uses reach it, so a
//! `POSITION` shared by two primitives, a time accessor shared by three
//! samplers, and a translation output shared by two nodes with the same
//! multiplier all still convert.
//!
//! # Absent `inverseBindMatrices`
//!
//! glTF's format default ("no `inverseBindMatrices`, every joint's bind is
//! identity") is unreachable here and would be unimplementable if it were
//! not: #280 refuses such a skin outright with `MissingInverseBinds`, and
//! materializing the array the way [`animsmith_core::scale::build_scale_candidate`]
//! does would append an accessor, a `bufferView` and buffer bytes — breaking
//! array identities, preserved-byte length equality, and
//! [`super::container`]'s no-length-change invariant at once. This module
//! therefore documents the case rather than mirroring core's materialization.

use super::bytes;
use super::{GltfScaleArtifact, GltfScaleRewriteError};
use crate::LoadError;
use crate::capability::{GltfScaleSource, declared};
use animsmith_core::scale::{ScaleOperation, ScalePlan, ScaleRequest, plan_scale};
use animsmith_core::{BoneId, Document};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

// --- The affected domain ----------------------------------------------------

/// The closed connected affected hierarchy of DESIGN.md Appendix D §D.2, in
/// raw source-node identity space, plus the declared common factor.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RestBindDomain {
    /// Source-node index of the scaled ancestor: the one closure member whose
    /// parent lies outside the closure.
    pub(crate) root: usize,
    /// Every source-node index in the closure, root included.
    pub(crate) closure: BTreeSet<usize>,
    /// The caller-declared common factor `s` the plan validated.
    pub(crate) factor: f64,
}

impl RestBindDomain {
    /// `s_parent(i)`: the multiplier this node's local translation, its
    /// `matrix` translation column, and its translation animation values and
    /// cubic tangents receive.
    pub(crate) fn parent_factor(&self, node: usize) -> f64 {
        if node != self.root && self.closure.contains(&node) {
            self.factor
        } else {
            1.0
        }
    }

    /// `s_i`: the factor this node's own basis correction removes, and so the
    /// multiplier its inverse-bind rows receive.
    pub(crate) fn node_factor(&self, node: usize) -> f64 {
        if self.closure.contains(&node) {
            self.factor
        } else {
            1.0
        }
    }

    /// `s_parent / s_i`: the multiplier this node's local scale — or its
    /// `matrix` linear part — receives.
    ///
    /// Exactly `1.0` for every affected node but the root, and for every
    /// unaffected node: IEEE-754 division is exact for `x / x` at any finite
    /// nonzero `x`, so the `!= 1.0` tests this module makes against it decide
    /// "this location changes" without a tolerance and without depending on
    /// the magnitude of `s`.
    pub(crate) fn local_scale_factor(&self, node: usize) -> f64 {
        self.parent_factor(node) / self.node_factor(node)
    }
}

/// Parent links, derived from raw `/nodes/*/children`.
///
/// This is deliberately *not* `SourceNodeAsset::parent_source_node_index`:
/// deriving the closure from the raw child arrays and then requiring it to
/// equal the plan's is what turns a projection that contradicts the source
/// into a typed refusal rather than a clean plan-build-prove cycle over the
/// wrong hierarchy.
fn raw_parents(root: &Map<String, Value>) -> Result<BTreeMap<usize, usize>, GltfScaleRewriteError> {
    let mut parents = BTreeMap::new();
    let nodes = root
        .get("nodes")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    for (node_index, node) in nodes.iter().enumerate() {
        for child in node
            .get("children")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            let child = as_index(Some(child)).ok_or_else(|| {
                LoadError::Malformed(format!(
                    "/nodes/{node_index}/children entry is not an index"
                ))
            })?;
            if parents.insert(child, node_index).is_some() {
                return Err(GltfScaleRewriteError::UnusableSourceHierarchy {
                    reason: "a node is named as a child by two parents",
                });
            }
        }
    }
    Ok(parents)
}

/// Re-derive the affected closure from raw JSON alone.
///
/// Same construction as `animsmith_core::scale`'s: the scaled root, every
/// joint of the selected skin, the ancestor path from each joint up to the
/// root, and every descendant of all of those. The skin is resolved by
/// `skins` array position, which is what a glTF source-skin index *is* — the
/// loader records `skin.index()` as `SourceSkinAsset::source_skin_index`.
fn raw_closure(
    root: &Map<String, Value>,
    parents: &BTreeMap<usize, usize>,
    source_skin_index: usize,
    source_root_node_index: usize,
) -> Result<BTreeSet<usize>, GltfScaleRewriteError> {
    let nodes = root
        .get("nodes")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if source_root_node_index >= nodes.len() {
        return Err(GltfScaleRewriteError::UnusableSourceHierarchy {
            reason: "the selected root node index is not a source node",
        });
    }
    let skin = root
        .get("skins")
        .and_then(Value::as_array)
        .and_then(|skins| skins.get(source_skin_index))
        .ok_or(GltfScaleRewriteError::UnusableSourceHierarchy {
            reason: "the selected skin index is not a source skin",
        })?;

    let mut closure = BTreeSet::from([source_root_node_index]);
    for joint in skin
        .get("joints")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        let joint = as_index(Some(joint)).ok_or_else(|| {
            LoadError::Malformed(format!(
                "/skins/{source_skin_index}/joints entry is not an index"
            ))
        })?;
        let mut cursor = joint;
        closure.insert(cursor);
        // Bounded by the node count: a raw child array can name a cycle, and
        // this walk must refuse rather than spin.
        for step in 0..=nodes.len() {
            if cursor == source_root_node_index {
                break;
            }
            if step == nodes.len() {
                return Err(GltfScaleRewriteError::UnusableSourceHierarchy {
                    reason: "a skin joint's parent chain is cyclic or unbounded",
                });
            }
            let Some(&parent) = parents.get(&cursor) else {
                return Err(GltfScaleRewriteError::UnusableSourceHierarchy {
                    reason: "a skin joint is not a descendant of the selected root node",
                });
            };
            closure.insert(parent);
            cursor = parent;
        }
    }

    let mut children: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (&child, &parent) in parents {
        children.entry(parent).or_default().push(child);
    }
    let mut queue: Vec<usize> = closure.iter().copied().collect();
    while let Some(node) = queue.pop() {
        for &child in children.get(&node).into_iter().flatten() {
            if closure.insert(child) {
                queue.push(child);
            }
        }
    }
    Ok(closure)
}

/// Every mapping between raw source-node identity and normalized
/// [`BoneId`], built defensively.
struct BoneProjection {
    bone_of_source: BTreeMap<usize, BoneId>,
    source_of_bone: BTreeMap<BoneId, usize>,
}

fn bone_projection(document: &Document) -> Result<BoneProjection, GltfScaleRewriteError> {
    let mut bone_of_source = BTreeMap::new();
    let mut source_of_bone = BTreeMap::new();
    for node in &document.assets.source_skeleton.nodes {
        let Some(bone) = node.bone else {
            continue;
        };
        bone_of_source.insert(node.source_node_index, bone);
        // Two source nodes claiming one bone makes the inverse map
        // last-write-wins, which would silently rewrite the wrong node's
        // JSON. `animsmith_core` validates that `source_node_index` is
        // unique; nothing validates `bone`.
        if source_of_bone
            .insert(bone, node.source_node_index)
            .is_some()
        {
            return Err(GltfScaleRewriteError::AmbiguousSourceNodeProjection { bone });
        }
    }
    Ok(BoneProjection {
        bone_of_source,
        source_of_bone,
    })
}

/// Require the raw-JSON closure, the plan's closure, and the normalized
/// skeleton's parent chain to describe the same hierarchy.
///
/// The plan speaks [`BoneId`] and walks
/// `SourceNodeAsset::parent_source_node_index`; this rewriter speaks source
/// node index and walks `/nodes/*/children`; sampling and proof walk
/// `Skeleton::parent`. Nothing in `animsmith_core` compares the last two, so
/// a document whose projection contradicts its own skeleton plans, builds and
/// proves cleanly today (issue #309). It must not also *rewrite* cleanly, so
/// all three are required to agree here before a byte is written.
fn cross_check_domain(
    document: &Document,
    plan: &ScalePlan,
    domain: &RestBindDomain,
    parents: &BTreeMap<usize, usize>,
    projection: &BoneProjection,
) -> Result<(), GltfScaleRewriteError> {
    let mut planned = BTreeSet::new();
    for &bone in plan.affected_nodes() {
        let source = *projection
            .source_of_bone
            .get(&bone)
            .ok_or(GltfScaleRewriteError::AmbiguousSourceNodeProjection { bone })?;
        planned.insert(source);
    }
    if planned != domain.closure {
        return Err(GltfScaleRewriteError::ClosureMismatch {
            planned: planned.into_iter().collect(),
            derived: domain.closure.iter().copied().collect(),
        });
    }

    let affected_bones: BTreeSet<BoneId> = plan.affected_nodes().iter().copied().collect();
    for &source_node_index in &domain.closure {
        let bone = *projection.bone_of_source.get(&source_node_index).ok_or(
            GltfScaleRewriteError::UnusableSourceHierarchy {
                reason: "an affected source node did not normalize to a skeleton bone",
            },
        )?;
        let skeleton_parent = document
            .skeleton
            .bones
            .get(bone)
            .ok_or(GltfScaleRewriteError::UnusableSourceHierarchy {
                reason: "an affected bone is out of range for the loaded skeleton",
            })?
            .parent;
        if source_node_index == domain.root {
            // The root's own parent lies outside the closure by construction.
            // What must not happen is the skeleton placing it *inside*: that
            // would make the root's `s_parent` `s` in every core-side
            // composition while this rewriter used `1`.
            if skeleton_parent.is_some_and(|parent| affected_bones.contains(&parent)) {
                return Err(GltfScaleRewriteError::ParentChainDisagreement { source_node_index });
            }
            continue;
        }
        let raw_parent = parents.get(&source_node_index).copied();
        let expected =
            raw_parent.and_then(|parent| projection.bone_of_source.get(&parent).copied());
        if skeleton_parent != expected {
            return Err(GltfScaleRewriteError::ParentChainDisagreement { source_node_index });
        }
    }
    Ok(())
}

// --- Accessor claims --------------------------------------------------------

/// Which components of one accessor element a rest/bind claim covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RestBindComponents {
    /// Every component: a `VEC3` translation sampler `output` (values and
    /// both `CUBICSPLINE` tangents alike) or a mesh `POSITION`.
    All,
    /// Column-major `MAT4` rows 0..2 — components `0,1,2, 4,5,6, 8,9,10,
    /// 12,13,14`. `B' = scale(s) * B` scales every output row and leaves the
    /// homogeneous row `3,7,11,15` alone.
    Mat4Rows,
}

impl RestBindComponents {
    pub(crate) fn covers(self, component: usize) -> bool {
        match self {
            Self::All => true,
            Self::Mat4Rows => component % 4 != 3,
        }
    }

    fn required_accessor_type(self) -> Option<&'static str> {
        match self {
            Self::All => None,
            Self::Mat4Rows => Some("MAT4"),
        }
    }
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
        if !self.components.covers(component) {
            return None;
        }
        let factor = self.factors.at(element);
        (factor != 1.0).then_some(factor)
    }

    /// Whether this claim changes nothing, so the accessor is not rewritten
    /// at all.
    pub(crate) fn is_identity(&self) -> bool {
        (0..self.count).all(|element| self.factors.at(element) == 1.0)
    }

    /// The single factor every element of this claim shares, when there is
    /// one.
    ///
    /// Authored `min`/`max` can only be converted by a single factor; a claim
    /// with mixed per-slot factors has no such conversion and its bounds are
    /// re-derived from the rewritten payload instead.
    pub(crate) fn uniform_factor(&self) -> Option<f64> {
        let first = self.factors.at(0);
        (1..self.count)
            .all(|element| self.factors.at(element) == first)
            .then_some(first)
    }

    /// The first element at which two claims on one accessor disagree.
    fn first_disagreement(&self, other: &Self) -> Option<usize> {
        if self.components != other.components {
            return Some(0);
        }
        (0..self.count.max(other.count))
            .find(|&element| self.factors.at(element) != other.factors.at(element))
    }
}

/// Build the accessor-to-factor map, refusing every disagreement.
///
/// Covers every scale-bearing accessor, factor-`1` ones included: an accessor
/// used as mesh `POSITION` *and* as an affected translation `output` is only
/// detectable because `POSITION` claims `1` here rather than being skipped.
pub(crate) fn collect_rest_bind_claims(
    root: &Map<String, Value>,
    domain: &RestBindDomain,
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

    for (mesh_index, mesh) in array(root, "meshes").iter().enumerate() {
        for (primitive_index, primitive) in mesh
            .get("primitives")
            .map_or(&[][..], value_array)
            .iter()
            .enumerate()
        {
            let Some(accessor_index) = as_index(
                primitive
                    .get("attributes")
                    .and_then(|attributes| attributes.get("POSITION")),
            ) else {
                continue;
            };
            let count = accessor_count(root, accessor_index)?;
            claim(
                accessor_index,
                RestBindClaim {
                    components: RestBindComponents::All,
                    factors: RestBindFactors::Uniform(1.0),
                    count,
                    location: format!(
                        "/meshes/{mesh_index}/primitives/{primitive_index}/attributes/POSITION"
                    ),
                },
                &mut claims,
            )?;
        }
    }

    for (skin_index, skin) in array(root, "skins").iter().enumerate() {
        let Some(accessor_index) = as_index(skin.get("inverseBindMatrices")) else {
            continue;
        };
        let count = accessor_count(root, accessor_index)?;
        let mut factors = Vec::with_capacity(count);
        for joint in skin.get("joints").map_or(&[][..], value_array) {
            let joint = as_index(Some(joint)).ok_or_else(|| {
                LoadError::Malformed(format!("/skins/{skin_index}/joints entry is not an index"))
            })?;
            factors.push(domain.node_factor(joint));
        }
        // Without this a joint list longer than the accessor drops the
        // surplus factors and a shorter one leaves the tail slots at `1`.
        //
        // It cannot fire through `rewrite_rest_bind`, and the gate-bypass
        // seam does not change that. `manifest_violations` re-derives
        // `InverseBindCountMismatch` from the manifest's own joint and
        // inverse-bind counts, so `capability_facts` sets
        // `inverse_bind_issues_present` for a bypassed source too and the
        // rewriter refuses it with `Capability` before it reads a skin. The
        // two #282 guards are reachable past the gate precisely because their
        // violations are inventory-time only and leave no manifest trace.
        // `a_skin_whose_joints_outnumber_its_inverse_binds_is_refused` pins
        // what this names; nothing can pin that it is wired.
        if factors.len() != count {
            return Err(GltfScaleRewriteError::UnrewritableAccessor {
                accessor_index,
                location: format!("/accessors/{accessor_index}"),
            });
        }
        claim(
            accessor_index,
            RestBindClaim {
                components: RestBindComponents::Mat4Rows,
                factors: RestBindFactors::PerElement(factors),
                count,
                location: format!("/skins/{skin_index}/inverseBindMatrices"),
            },
            &mut claims,
        )?;
    }

    for (animation_index, animation) in array(root, "animations").iter().enumerate() {
        let samplers = animation.get("samplers").map_or(&[][..], value_array);
        for (channel_index, channel) in animation
            .get("channels")
            .map_or(&[][..], value_array)
            .iter()
            .enumerate()
        {
            let target = channel.get("target");
            if target
                .and_then(|target| target.get("path"))
                .and_then(Value::as_str)
                != Some("translation")
            {
                continue;
            }
            let Some(node) = as_index(target.and_then(|target| target.get("node"))) else {
                continue;
            };
            let Some(accessor_index) = as_index(channel.get("sampler"))
                .and_then(|sampler| samplers.get(sampler))
                .and_then(|sampler| as_index(sampler.get("output")))
            else {
                continue;
            };
            let count = accessor_count(root, accessor_index)?;
            claim(
                accessor_index,
                RestBindClaim {
                    components: RestBindComponents::All,
                    factors: RestBindFactors::Uniform(domain.parent_factor(node)),
                    count,
                    location: format!("/animations/{animation_index}/channels/{channel_index}"),
                },
                &mut claims,
            )?;
        }
    }
    Ok(claims)
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
        match self {
            Self::Translation(factor) | Self::Scale(factor) => Some(factor),
            Self::Matrix {
                linear,
                translation,
            } => match component {
                0..=2 | 4..=6 | 8..=10 => Some(linear),
                12..=14 => Some(translation),
                _ => None,
            },
        }
    }
}

/// Every node transform member this plan rebases, in source node order.
///
/// A member whose multiplier is exactly `1.0` is not emitted, which is what
/// makes a declared factor of one a deterministic no-op rather than a
/// re-render of every affected node's transform.
pub(crate) fn collect_node_rebases(
    root: &Map<String, Value>,
    domain: &RestBindDomain,
) -> Vec<(usize, NodeRebase)> {
    let mut out = Vec::new();
    let nodes = array(root, "nodes");
    for &node_index in &domain.closure {
        let Some(node) = nodes.get(node_index) else {
            continue;
        };
        let s_parent = domain.parent_factor(node_index);
        let s_local = domain.local_scale_factor(node_index);
        if declared(node, "matrix").is_some() {
            if s_parent != 1.0 || s_local != 1.0 {
                out.push((
                    node_index,
                    NodeRebase::Matrix {
                        linear: s_local,
                        translation: s_parent,
                    },
                ));
            }
            continue;
        }
        if s_parent != 1.0 && declared(node, "translation").is_some() {
            out.push((node_index, NodeRebase::Translation(s_parent)));
        }
        if s_local != 1.0 {
            out.push((node_index, NodeRebase::Scale(s_local)));
        }
    }
    out
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
/// factor, affected scale animation, malformed track or instance data —
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
    let manifest = source.manifest();
    let facts = super::capability_facts(manifest);
    if !facts.is_supported() {
        let violations = super::manifest_violations(manifest);
        let count = violations.len();
        return Err(GltfScaleRewriteError::Capability { violations, count });
    }
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

    let root = source
        .raw_json()
        .as_object()
        .ok_or_else(|| LoadError::Malformed("top-level glTF JSON is not an object".into()))?;
    super::reject_out_of_contract_nodes(root)?;

    let parents = raw_parents(root)?;
    let domain = RestBindDomain {
        root: source_root_node_index,
        closure: raw_closure(root, &parents, source_skin_index, source_root_node_index)?,
        factor: expected_factor,
    };
    let projection = bone_projection(source.document())?;
    cross_check_domain(source.document(), &plan, &domain, &parents, &projection)?;

    let claims = collect_rest_bind_claims(root, &domain)?;
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
                claim.components.required_accessor_type(),
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
    for (node_index, rebase) in collect_node_rebases(root, &domain) {
        rewrite_node_member(&mut json, node_index, rebase)?;
        rewritten_json_pointers.push(format!("/nodes/{node_index}/{}", rebase.member()));
    }
    for (&accessor_index, claim) in &rewritten {
        rewritten_json_pointers.extend(super::rewrite_accessor_bounds_with(
            &mut json,
            accessor_index,
            &|component| claim.components.covers(component),
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
    let affected_source_skins = affected_skins(root, &domain)?;
    let out = super::container::assemble(manifest, &json, &buffers, &modified)?;
    Ok(GltfScaleArtifact {
        container: manifest.container,
        bytes: out,
        rewritten_accessors: rewritten.keys().copied().collect(),
        rewritten_json_pointers,
        reencoded_buffers,
        // The raw closure this rewrite was driven by, in the same source-node
        // index space `source_root_node_index` selected it with.
        affected_source_nodes: domain.closure.iter().copied().collect(),
        affected_source_skins,
        declared_factor: expected_factor,
        operation,
    })
}

/// Source-skin indices with at least one joint inside `domain`, ascending.
///
/// This is the same predicate [`collect_rest_bind_claims`] applies per slot:
/// a skin is affected exactly when some slot of its `inverseBindMatrices`
/// takes a factor other than one. A skin the closure does not reach at all is
/// left out, which is what makes its binds provably untouched evidence rather
/// than an unexplained absence.
fn affected_skins(
    root: &Map<String, Value>,
    domain: &RestBindDomain,
) -> Result<Vec<usize>, GltfScaleRewriteError> {
    let mut affected = Vec::new();
    for (skin_index, skin) in array(root, "skins").iter().enumerate() {
        let mut touched = false;
        for joint in skin.get("joints").map_or(&[][..], value_array) {
            let joint = as_index(Some(joint)).ok_or_else(|| {
                LoadError::Malformed(format!("/skins/{skin_index}/joints entry is not an index"))
            })?;
            touched |= domain.closure.contains(&joint);
        }
        if touched {
            affected.push(skin_index);
        }
    }
    Ok(affected)
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

// --- Small shared readers ---------------------------------------------------

fn array<'a>(root: &'a Map<String, Value>, key: &str) -> &'a [Value] {
    root.get(key).map_or(&[][..], value_array)
}

fn value_array(value: &Value) -> &[Value] {
    value.as_array().map_or(&[][..], Vec::as_slice)
}

fn as_index(value: Option<&Value>) -> Option<usize> {
    value?.as_u64()?.try_into().ok()
}

fn accessor_count(
    root: &Map<String, Value>,
    accessor_index: usize,
) -> Result<usize, GltfScaleRewriteError> {
    array(root, "accessors")
        .get(accessor_index)
        .and_then(|accessor| accessor.get("count"))
        .and_then(Value::as_u64)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or(GltfScaleRewriteError::UnrewritableAccessor {
            accessor_index,
            location: format!("/accessors/{accessor_index}"),
        })
}

fn number(value: f32, location: &str) -> Result<Value, GltfScaleRewriteError> {
    super::number(value, location)
}

#[cfg(test)]
mod tests {
    //! Unit tests for the two things `rewrite_rest_bind` decides before it
    //! writes a byte: what the affected hierarchy *is*, and whether the three
    //! independent descriptions of it agree.
    //!
    //! The agreement checks cannot be falsified by any glTF byte sequence.
    //! The raw child arrays, `SourceNodeAsset::parent_source_node_index` and
    //! `Skeleton::parent` are all derived by this crate's own `topology` from
    //! one parsed document, so no source makes them disagree — which is
    //! exactly why issue #309 is a latent gap rather than a live bug, and
    //! exactly why these are classification tests over a mutated `Document`
    //! rather than end-to-end ones. What they pin is that each disagreement
    //! is *named*, and — the direction that has caught more in this lane —
    //! that the unmutated document still passes all three. Their *wiring* is
    //! pinned separately, through `capability::scale_source_with_document`:
    //! the seam hands the rewriter a real source whose normalized projection
    //! contradicts its own bytes, which is the one relaxation gate bypass
    //! cannot supply.
    //!
    //! One check in this module has neither. `collect_rest_bind_claims`'s
    //! joint-count guard is unreachable behind #280's
    //! `InverseBindCountMismatch`, which `manifest_violations` re-derives
    //! from the manifest — so `rewrite_rest_bind`'s own `Capability` re-check
    //! refuses such a source before it reads a skin whether or not the gate
    //! was bypassed. It is classification-tested only, and said so here
    //! rather than left looking end-to-end. Its mirror in
    //! `rest_bind_proof::expected_rebases` is in the same position and
    //! documented there.

    use super::*;
    use crate::preflight_scale_source_bytes;
    use animsmith_core::scale::plan_scale;
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

    fn root_object(source: &GltfScaleSource) -> Map<String, Value> {
        source
            .raw_json()
            .as_object()
            .expect("top-level object")
            .clone()
    }

    /// Run all three agreement checks over `document`, with the raw hierarchy
    /// and the closure taken from the unmodified source.
    fn check(document: &Document, plan: &ScalePlan) -> Result<(), GltfScaleRewriteError> {
        let source = source();
        let root = root_object(&source);
        let parents = raw_parents(&root).expect("raw parents");
        let domain = RestBindDomain {
            root: 0,
            closure: raw_closure(&root, &parents, 0, 0).expect("raw closure"),
            factor: 0.01,
        };
        let projection = bone_projection(document)?;
        cross_check_domain(document, plan, &domain, &parents, &projection)
    }

    #[test]
    fn the_three_hierarchy_descriptions_agree_for_an_ordinary_document() {
        // The must-not-over-reject direction, and the reason the mutations
        // below can be attributed: without it each could be "rejected" by a
        // check that rejects everything.
        let source = source();
        let plan = plan(&source);
        check(source.document(), &plan).expect("an ordinary document passes all three checks");
        // And the closure itself is the §D.2 one: root, joint, attachment —
        // and not the mesh holder, which is a sibling scene root.
        let root = root_object(&source);
        let parents = raw_parents(&root).expect("raw parents");
        assert_eq!(
            raw_closure(&root, &parents, 0, 0).expect("raw closure"),
            BTreeSet::from([0, 1, 2])
        );
    }

    #[test]
    fn two_source_nodes_claiming_one_bone_are_refused() {
        let source = source();
        let plan = plan(&source);
        let mut document = source.document().clone();
        document.assets.source_skeleton.nodes[2].bone = Some(1);
        match check(&document, &plan) {
            Err(GltfScaleRewriteError::AmbiguousSourceNodeProjection { bone }) => {
                assert_eq!(bone, 1);
            }
            other => panic!("expected AmbiguousSourceNodeProjection, got {other:?}"),
        }
    }

    #[test]
    fn a_plan_closure_the_raw_hierarchy_does_not_produce_is_refused() {
        // Re-point the attachment's source parent at the holder, so the
        // plan's closure loses node 2 while the raw child arrays keep it.
        let source = source();
        let mut document = source.document().clone();
        document.assets.source_skeleton.nodes[2].parent_source_node_index = Some(3);
        let plan = plan_scale(&ScaleRequest {
            operation: ScaleOperation::RestBindUniformScale {
                source_skin_index: 0,
                source_root_node_index: 0,
                expected_factor: 0.01,
            },
            document: &document,
            capability: &super::super::capability_facts(source.manifest()),
        })
        .expect("the contradicting projection still plans cleanly — that is issue #309");
        match check(&document, &plan) {
            Err(GltfScaleRewriteError::ClosureMismatch { planned, derived }) => {
                assert_eq!(planned, vec![0, 1]);
                assert_eq!(derived, vec![0, 1, 2]);
            }
            other => panic!("expected ClosureMismatch, got {other:?}"),
        }
    }

    #[test]
    fn a_skeleton_parent_that_contradicts_the_raw_children_is_refused() {
        let source = source();
        let plan = plan(&source);
        let mut document = source.document().clone();
        // Bone 2 is source node 2, whose raw parent is node 1 (bone 1).
        document.skeleton.bones[2].parent = Some(0);
        match check(&document, &plan) {
            Err(GltfScaleRewriteError::ParentChainDisagreement { source_node_index }) => {
                assert_eq!(source_node_index, 2);
            }
            other => panic!("expected ParentChainDisagreement, got {other:?}"),
        }
    }

    #[test]
    fn a_skeleton_that_places_the_closure_root_under_an_affected_node_is_refused() {
        // The root's `s_parent` is one *because* its parent is outside the
        // closure. A skeleton that says otherwise would make every core-side
        // composition disagree with this rewriter about the one node whose
        // local scale changes.
        let source = source();
        let plan = plan(&source);
        let mut document = source.document().clone();
        document.skeleton.bones[0].parent = Some(2);
        match check(&document, &plan) {
            Err(GltfScaleRewriteError::ParentChainDisagreement { source_node_index }) => {
                assert_eq!(source_node_index, 0);
            }
            other => panic!("expected ParentChainDisagreement, got {other:?}"),
        }
    }

    #[test]
    fn a_raw_hierarchy_the_closure_cannot_be_derived_from_is_refused() {
        let source = source();
        let root = root_object(&source);
        let parents = raw_parents(&root).expect("raw parents");
        // The holder is a sibling scene root, so a skin whose joint is the
        // holder has no ancestor path up to node 0.
        let mut detached = root.clone();
        detached["skins"] = json!([{ "joints": [3], "inverseBindMatrices": 3 }]);
        match raw_closure(&detached, &parents, 0, 0) {
            Err(GltfScaleRewriteError::UnusableSourceHierarchy { reason }) => {
                assert_eq!(
                    reason,
                    "a skin joint is not a descendant of the selected root node"
                );
            }
            other => panic!("expected UnusableSourceHierarchy, got {other:?}"),
        }
        // A node named as a child by two parents has no single parent link to
        // walk, so the closure cannot be derived at all.
        let mut two_parents = root.clone();
        two_parents["nodes"][3]["children"] = json!([2]);
        match raw_parents(&two_parents) {
            Err(GltfScaleRewriteError::UnusableSourceHierarchy { reason }) => {
                assert_eq!(reason, "a node is named as a child by two parents");
            }
            other => panic!("expected UnusableSourceHierarchy, got {other:?}"),
        }
        // A cyclic child chain must terminate the ancestor walk rather than
        // spin. glTF forbids it and the typed parse does not, so this walk is
        // the layer that has to bound itself.
        let mut cyclic = root.clone();
        cyclic["nodes"][2]["children"] = json!([0]);
        let cyclic_parents = raw_parents(&cyclic).expect("each node still has one parent");
        // Rooted at the holder, so the joint's walk up the 0 -> 2 -> 1 -> 0
        // cycle never reaches the root and never runs out of parents either.
        match raw_closure(&cyclic, &cyclic_parents, 0, 3) {
            Err(GltfScaleRewriteError::UnusableSourceHierarchy { reason }) => {
                assert_eq!(reason, "a skin joint's parent chain is cyclic or unbounded");
            }
            other => panic!("expected a bounded refusal, got {other:?}"),
        }
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
    fn rewrite_rest_bind_still_calls_the_hierarchy_cross_check() {
        // The classification tests above call `cross_check_domain` directly,
        // which pins *what* each disagreement is named and leaves the call
        // site unevaluated: deleting `cross_check_domain(...)?` from
        // `rewrite_rest_bind` changes no observable behaviour without this.
        // `capability::scale_source_with_document` is the `cfg(test)`-only
        // seam that supplies the relaxation no glTF byte sequence can — a
        // source whose normalized skeleton contradicts its own raw children.
        let source = source();
        let mut document = source.document().clone();
        // Bone 2 is source node 2, whose raw parent is node 1 (bone 1).
        document.skeleton.bones[2].parent = Some(0);
        let contradicting = crate::capability::scale_source_with_document(source, document);
        match rewrite_rest_bind(&contradicting, 0, 0, 0.01) {
            Err(GltfScaleRewriteError::ParentChainDisagreement { source_node_index }) => {
                assert_eq!(source_node_index, 2);
            }
            other => {
                panic!("the wired cross-check must refuse a contradicting skeleton, got {other:?}")
            }
        }
    }

    #[test]
    fn the_wired_cross_check_still_accepts_an_untouched_document() {
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
    fn a_skin_whose_joints_outnumber_its_inverse_binds_is_refused() {
        // The fourth check of this module that no input can reach, and for
        // the same reason as the three above: it is one layer's defence
        // against another layer being relaxed. Unlike the two #282 guards the
        // gate-bypass seam does not help — `InverseBindCountMismatch` is
        // re-derivable from the manifest, so `rewrite_rest_bind`'s own
        // `Capability` re-check refuses a bypassed source too. So this pins
        // what the guard names over a doctored raw tree, and nothing pins
        // that it is wired.
        let source = source();
        let root = root_object(&source);
        let parents = raw_parents(&root).expect("raw parents");
        let domain = RestBindDomain {
            root: 0,
            closure: raw_closure(&root, &parents, 0, 0).expect("raw closure"),
            factor: 0.01,
        };
        let mut doctored = root.clone();
        doctored["skins"][0]["joints"] = json!([1, 2]);
        match collect_rest_bind_claims(&doctored, &domain) {
            Err(GltfScaleRewriteError::UnrewritableAccessor {
                accessor_index,
                location,
            }) => {
                assert_eq!(accessor_index, 3);
                assert_eq!(location, "/accessors/3");
            }
            other => panic!("expected UnrewritableAccessor, got {other:?}"),
        }
        // The must-not-over-reject direction, without which the above could
        // be a guard that refuses every skin.
        collect_rest_bind_claims(&root, &domain).expect("the fixture's own skin claims cleanly");
    }

    #[test]
    fn the_mat4_row_mask_covers_exactly_the_three_output_rows() {
        let covered: Vec<usize> = (0..16)
            .filter(|&component| RestBindComponents::Mat4Rows.covers(component))
            .collect();
        assert_eq!(covered, vec![0, 1, 2, 4, 5, 6, 8, 9, 10, 12, 13, 14]);
        assert!((0..16).all(|component| RestBindComponents::All.covers(component)));
    }

    #[test]
    fn the_local_scale_multiplier_is_exactly_one_off_the_closure_root() {
        // Every "does this location change" decision in this module is an
        // exact `!= 1.0` test against this quantity, so it must be exactly
        // one — not one within a tolerance — at every factor the policy
        // admits.
        for factor in [1.0, 0.01, 1e-9, 1e9, 3.0 / 7.0] {
            let domain = RestBindDomain {
                root: 0,
                closure: BTreeSet::from([0, 1, 2]),
                factor,
            };
            for node in [1, 2] {
                assert_eq!(domain.local_scale_factor(node), 1.0, "factor {factor}");
                assert_eq!(domain.parent_factor(node), factor, "factor {factor}");
            }
            assert_eq!(domain.parent_factor(0), 1.0, "factor {factor}");
            assert_eq!(
                domain.local_scale_factor(0),
                1.0 / factor,
                "factor {factor}"
            );
            // A node outside the closure is untouched in both domains.
            assert_eq!(domain.parent_factor(9), 1.0);
            assert_eq!(domain.local_scale_factor(9), 1.0);
            assert_eq!(domain.node_factor(9), 1.0);
        }
    }
}
