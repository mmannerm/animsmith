//! The DESIGN.md Appendix D §D.2 domain table, as data.
//!
//! Two independent tables live here:
//!
//! 1. The **accessor rewrite set** and the **JSON pointer rewrite set** —
//!    which raw glTF locations carry a length under a whole-document
//!    linear-unit conversion, and how each converts.
//! 2. The **length-field handler registry** plus its two companion decision
//!    tables. Appendix D §D.2 requires that "every supported camera, light,
//!    collision, or extension length" is converted "through a field-specific
//!    handler". This slice registers no handlers, so the reachable set is
//!    exactly the domains #280's preflight already rejects. Keeping the set
//!    as data rather than code means a future handler is an entry, and a
//!    length field with no entry is a located rejection rather than a silent
//!    skip.

use crate::capability::declared;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

// --- Accessor rewrite set -------------------------------------------------

/// How one dense `f32` accessor's components convert under the declared
/// factor `q`.
///
/// A rule is expressed per *component index within one element*, so it
/// applies unchanged to the accessor's packed buffer payload and to its
/// authored `min`/`max` arrays (which carry exactly one entry per element
/// component).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccessorRule {
    /// Every component of every element is a length: mesh `POSITION`, and
    /// every element of a translation sampler `output`.
    ///
    /// A `CUBICSPLINE` translation output needs no special case: it stores
    /// `3N` `VEC3`s as `[in-tangent, value, out-tangent]` per key and all
    /// `3N` of them are lengths, so the whole-accessor rule is already the
    /// Appendix D.2 tangent rule.
    AllComponents,
    /// Column-major `MAT4`: only the translation column — components 12, 13
    /// and 14 — is a length. `M' = U M U^-1` for a uniform `U = scale(q)`
    /// leaves the 3x3 linear part and the homogeneous component 15 alone.
    Mat4TranslationColumn,
}

impl AccessorRule {
    /// Whether component `component` of one element is multiplied by `q`.
    pub(crate) fn scales_component(self, component: usize) -> bool {
        match self {
            Self::AllComponents => true,
            Self::Mat4TranslationColumn => matches!(component, 12..=14),
        }
    }

    /// The accessor `type` spelling this rule requires, or `None` when the
    /// rule is type-agnostic.
    pub(crate) fn required_accessor_type(self) -> Option<&'static str> {
        match self {
            Self::AllComponents => None,
            Self::Mat4TranslationColumn => Some("MAT4"),
        }
    }
}

/// Components per element for a glTF accessor `type`.
///
/// For `f32` component types this is also the number of stored floats per
/// element: glTF's matrix column padding rounds each column up to four
/// bytes, which is a no-op for four-byte components.
pub(crate) fn components_per_element(accessor_type: &str) -> Option<usize> {
    Some(match accessor_type {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" | "MAT2" => 4,
        "MAT3" => 9,
        "MAT4" => 16,
        _ => return None,
    })
}

/// Build the whole-document accessor rewrite set, keyed by **unique accessor
/// index**.
///
/// This is the aliasing guard. One accessor index is routinely reachable
/// through several logical uses — a `POSITION` shared by two primitives is
/// both common and legal, and #280 does not reject it because it is a single
/// map entry with a single use kind. Iterating logical uses and scaling in
/// place would multiply a shared accessor by `q^2` or `q^3`, and nothing
/// downstream catches that reliably. Copy-on-write is not the fix either:
/// splitting a shared accessor changes `bufferViews`/`accessors` array
/// lengths and destroys the preserved array identities.
///
/// So the rewrite set is a map from accessor index to rule, and the rewriter
/// walks that map. Two uses that agree on the rule collapse into one entry;
/// two that disagree are an error rather than a last-write-wins choice. That
/// disagreement cannot arise today (a `MAT4` inverse bind and a `VEC3`
/// `POSITION`/translation output are type-disjoint, and #280 rejects an
/// accessor shared between scale-bearing and dimensionless semantics), but
/// relying on that accident is exactly how the `q^2` bug comes back.
///
/// # Errors
///
/// Returns the accessor index whose logical uses disagree on a rule.
pub(crate) fn collect_accessor_rules(
    root: &Map<String, Value>,
) -> Result<BTreeMap<usize, AccessorRule>, usize> {
    let mut rules: BTreeMap<usize, AccessorRule> = BTreeMap::new();
    let mut insert = |index: Option<usize>, rule: AccessorRule| -> Result<(), usize> {
        let Some(index) = index else { return Ok(()) };
        match rules.insert(index, rule) {
            Some(previous) if previous != rule => Err(index),
            _ => Ok(()),
        }
    };

    if let Some(meshes) = root.get("meshes").and_then(Value::as_array) {
        for mesh in meshes {
            let Some(primitives) = mesh.get("primitives").and_then(Value::as_array) else {
                continue;
            };
            for primitive in primitives {
                let position = primitive
                    .get("attributes")
                    .and_then(|attributes| attributes.get("POSITION"));
                insert(as_index(position), AccessorRule::AllComponents)?;
            }
        }
    }
    if let Some(skins) = root.get("skins").and_then(Value::as_array) {
        for skin in skins {
            insert(
                as_index(skin.get("inverseBindMatrices")),
                AccessorRule::Mat4TranslationColumn,
            )?;
        }
    }
    if let Some(animations) = root.get("animations").and_then(Value::as_array) {
        for animation in animations {
            let samplers = animation
                .get("samplers")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            let channels = animation
                .get("channels")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            for channel in channels {
                let path = channel
                    .get("target")
                    .and_then(|target| target.get("path"))
                    .and_then(Value::as_str);
                if path != Some("translation") {
                    continue;
                }
                let Some(sampler) = as_index(channel.get("sampler")).and_then(|i| samplers.get(i))
                else {
                    continue;
                };
                insert(as_index(sampler.get("output")), AccessorRule::AllComponents)?;
            }
        }
    }
    Ok(rules)
}

/// How one raw JSON numeric array converts under `q`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JsonArrayRule {
    /// Every entry is a length (a node's three-component `translation`).
    AllComponents,
    /// Column-major 16-entry `matrix`: entries 12, 13 and 14 only.
    Mat4TranslationColumn,
}

impl JsonArrayRule {
    /// Required entry count for this rule.
    pub(crate) fn expected_len(self) -> usize {
        match self {
            Self::AllComponents => 3,
            Self::Mat4TranslationColumn => 16,
        }
    }

    /// Whether entry `component` is multiplied by `q`.
    pub(crate) fn scales_component(self, component: usize) -> bool {
        match self {
            Self::AllComponents => true,
            Self::Mat4TranslationColumn => matches!(component, 12..=14),
        }
    }
}

/// Every raw JSON location that carries a length, in source array order.
///
/// Node `rotation` and `scale` are deliberately absent: `M' = U M U^-1`
/// leaves both untouched for a uniform `U`.
///
/// A member authored as JSON `null` declares no transform — see
/// [`crate::capability::declared`] — and selecting one here would hand
/// [`super::rewrite_json_array`] a `null` to multiply, which it can only
/// report as a malformed source.
pub(crate) fn collect_json_rewrites(root: &Map<String, Value>) -> Vec<(String, JsonArrayRule)> {
    let mut out = Vec::new();
    let Some(nodes) = root.get("nodes").and_then(Value::as_array) else {
        return out;
    };
    for (node_index, node) in nodes.iter().enumerate() {
        if declared(node, "translation").is_some() {
            out.push((
                format!("/nodes/{node_index}/translation"),
                JsonArrayRule::AllComponents,
            ));
        }
        if declared(node, "matrix").is_some() {
            out.push((
                format!("/nodes/{node_index}/matrix"),
                JsonArrayRule::Mat4TranslationColumn,
            ));
        }
    }
    out
}

// --- Length-field handler registry ----------------------------------------

/// One registered length field and the handler that converts it.
///
/// `owner` is a JSON pointer with every array index replaced by `{}`, so one
/// entry covers every element of an array.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LengthFieldHandler {
    /// Normalized JSON pointer of the object that owns the member.
    pub(crate) owner: &'static str,
    /// Member name inside that object.
    pub(crate) member: &'static str,
}

/// Registered length-field handlers.
///
/// **Deliberately empty.** Every domain that would need an entry — camera
/// frustum distances and `KHR_lights_punctual` `range` — is rejected by
/// #280's preflight before a [`crate::GltfScaleSource`] exists, so
/// registering a handler here now would be untestable code claiming a
/// capability the boundary does not have. [`unhandled_length_fields`] proves
/// the empty registry rejects.
pub(crate) const LENGTH_FIELD_HANDLERS: &[LengthFieldHandler] = &[];

/// One length-bearing member with no registered handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UnhandledLengthField {
    /// Normalized JSON pointer of the object that owns the member.
    pub(crate) owner: &'static str,
    /// Member name inside that object.
    pub(crate) member: &'static str,
}

/// Raw glTF members that carry a length this slice cannot convert.
///
/// Presence of any of these is a located rejection, never a silent skip: a
/// document whose camera near plane stayed at `0.1` while every coordinate
/// around it moved by `q` is not a converted document.
pub(crate) const UNHANDLED_LENGTH_FIELDS: &[UnhandledLengthField] = &[
    UnhandledLengthField {
        owner: "/cameras/{}/orthographic",
        member: "xmag",
    },
    UnhandledLengthField {
        owner: "/cameras/{}/orthographic",
        member: "ymag",
    },
    UnhandledLengthField {
        owner: "/cameras/{}/orthographic",
        member: "zfar",
    },
    UnhandledLengthField {
        owner: "/cameras/{}/orthographic",
        member: "znear",
    },
    UnhandledLengthField {
        owner: "/cameras/{}/perspective",
        member: "zfar",
    },
    UnhandledLengthField {
        owner: "/cameras/{}/perspective",
        member: "znear",
    },
    UnhandledLengthField {
        owner: "/extensions/KHR_lights_punctual/lights/{}",
        member: "range",
    },
];

/// One member whose quantity was inspected and found *not* to be a length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DimensionlessField {
    /// Normalized JSON pointer of the object that owns the member.
    pub(crate) owner: &'static str,
    /// Member name inside that object.
    pub(crate) member: &'static str,
    /// Why the member is dimensionless, recorded so the decision is
    /// reviewable rather than implied by the absence of code.
    pub(crate) reason: &'static str,
}

/// Members that look convertible but are not.
///
/// This table records decisions; nothing reads it at rewrite time. It exists
/// so "we did not multiply light `intensity`" is a reviewed statement rather
/// than an omission — `intensity` is candela (point/spot) or lux
/// (directional), neither of which changes when the linear unit does.
pub(crate) const DIMENSIONLESS_FIELDS: &[DimensionlessField] = &[
    DimensionlessField {
        owner: "/cameras/{}/perspective",
        member: "aspectRatio",
        reason: "ratio of two lengths",
    },
    DimensionlessField {
        owner: "/cameras/{}/perspective",
        member: "yfov",
        reason: "angle in radians",
    },
    DimensionlessField {
        owner: "/extensions/KHR_lights_punctual/lights/{}",
        member: "color",
        reason: "normalized linear RGB",
    },
    DimensionlessField {
        owner: "/extensions/KHR_lights_punctual/lights/{}",
        member: "intensity",
        reason: "candela for point/spot, lux for directional",
    },
    DimensionlessField {
        owner: "/materials/{}",
        member: "alphaCutoff",
        reason: "alpha threshold",
    },
    DimensionlessField {
        owner: "/materials/{}/normalTexture",
        member: "scale",
        reason: "tangent-space normal strength multiplier",
    },
    DimensionlessField {
        owner: "/materials/{}/occlusionTexture",
        member: "strength",
        reason: "occlusion blend weight",
    },
    DimensionlessField {
        owner: "/meshes/{}",
        member: "weights",
        reason: "morph weights are blend coefficients",
    },
    DimensionlessField {
        owner: "/nodes/{}",
        member: "rotation",
        reason: "unit quaternion; U M U^-1 leaves the linear part alone",
    },
    DimensionlessField {
        owner: "/nodes/{}",
        member: "scale",
        reason: "ratio; U M U^-1 leaves the linear part alone",
    },
    DimensionlessField {
        owner: "/nodes/{}",
        member: "weights",
        reason: "morph weights are blend coefficients",
    },
];

/// Every length-bearing raw JSON location the registry cannot convert, as
/// JSON pointers in lexical order.
///
/// An entry covered by [`LENGTH_FIELD_HANDLERS`] is not reported. With an
/// empty registry every entry of [`UNHANDLED_LENGTH_FIELDS`] present in the
/// source is reported, which is precisely the "prove the empty registry
/// rejects" obligation.
pub(crate) fn unhandled_length_fields(root: &Value) -> Vec<String> {
    let mut out = Vec::new();
    walk_length_fields(root, "", &mut out);
    out.sort();
    out.dedup();
    out
}

fn walk_length_fields(value: &Value, pointer: &str, out: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            let owner = normalize_pointer(pointer);
            for (key, child) in object {
                let child_pointer = format!("{pointer}/{}", escape_token(key));
                if is_unhandled_length_field(&owner, key) {
                    out.push(child_pointer.clone());
                }
                walk_length_fields(child, &child_pointer, out);
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                walk_length_fields(child, &format!("{pointer}/{index}"), out);
            }
        }
        _ => {}
    }
}

fn is_unhandled_length_field(owner: &str, member: &str) -> bool {
    let handled = LENGTH_FIELD_HANDLERS
        .iter()
        .any(|handler| handler.owner == owner && handler.member == member);
    // A member recorded as dimensionless is never reported: the decision that
    // it carries no length was made in [`DIMENSIONLESS_FIELDS`] with a stated
    // reason, so it is skipped by record rather than by omission. The two
    // tables must stay disjoint, which
    // `the_two_decision_tables_are_disjoint_and_documented` pins.
    let dimensionless = DIMENSIONLESS_FIELDS
        .iter()
        .any(|field| field.owner == owner && field.member == member);
    !handled
        && !dimensionless
        && UNHANDLED_LENGTH_FIELDS
            .iter()
            .any(|field| field.owner == owner && field.member == member)
}

/// Replace every all-digit pointer token with `{}` so one table entry covers
/// every element of an array.
fn normalize_pointer(pointer: &str) -> String {
    if pointer.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(pointer.len());
    for token in pointer.split('/').skip(1) {
        out.push('/');
        if !token.is_empty() && token.bytes().all(|byte| byte.is_ascii_digit()) {
            out.push_str("{}");
        } else {
            out.push_str(token);
        }
    }
    out
}

fn escape_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn as_index(value: Option<&Value>) -> Option<usize> {
    value?.as_u64()?.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rules(value: &Value) -> BTreeMap<usize, AccessorRule> {
        collect_accessor_rules(value.as_object().expect("object")).expect("no rule conflict")
    }

    #[test]
    fn a_position_shared_by_two_primitives_is_one_rewrite_entry() {
        let value = json!({
            "meshes": [
                { "primitives": [
                    { "attributes": { "POSITION": 7, "NORMAL": 1 } },
                    { "attributes": { "POSITION": 7, "NORMAL": 2 } }
                ] },
                { "primitives": [{ "attributes": { "POSITION": 7 } }] }
            ]
        });
        assert_eq!(
            rules(&value).into_iter().collect::<Vec<_>>(),
            vec![(7, AccessorRule::AllComponents)]
        );
    }

    #[test]
    fn each_domain_contributes_its_own_rule_and_dimensionless_uses_contribute_none() {
        let value = json!({
            "meshes": [{ "primitives": [{
                "attributes": { "POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2 },
                "indices": 3
            }] }],
            "skins": [{ "joints": [0], "inverseBindMatrices": 4, "skeleton": 0 }],
            "animations": [{
                "samplers": [
                    { "input": 5, "output": 6 },
                    { "input": 5, "output": 7 },
                    { "input": 5, "output": 8 }
                ],
                "channels": [
                    { "sampler": 0, "target": { "node": 0, "path": "translation" } },
                    { "sampler": 1, "target": { "node": 0, "path": "rotation" } },
                    { "sampler": 2, "target": { "node": 0, "path": "scale" } }
                ]
            }]
        });
        assert_eq!(
            rules(&value).into_iter().collect::<Vec<_>>(),
            vec![
                (0, AccessorRule::AllComponents),
                (4, AccessorRule::Mat4TranslationColumn),
                (6, AccessorRule::AllComponents),
            ]
        );
    }

    #[test]
    fn disagreeing_rules_on_one_accessor_are_reported_by_index() {
        let value = json!({
            "meshes": [{ "primitives": [{ "attributes": { "POSITION": 3 } }] }],
            "skins": [{ "joints": [0], "inverseBindMatrices": 3 }]
        });
        assert_eq!(
            collect_accessor_rules(value.as_object().expect("object")),
            Err(3)
        );
    }

    #[test]
    fn json_rewrites_cover_translation_and_matrix_nodes_only() {
        let value = json!({
            "nodes": [
                { "translation": [1, 2, 3], "rotation": [0, 0, 0, 1], "scale": [2, 2, 2] },
                { "matrix": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 4, 5, 6, 1] },
                { "name": "bare" }
            ]
        });
        assert_eq!(
            collect_json_rewrites(value.as_object().expect("object")),
            vec![
                (
                    "/nodes/0/translation".to_owned(),
                    JsonArrayRule::AllComponents
                ),
                (
                    "/nodes/1/matrix".to_owned(),
                    JsonArrayRule::Mat4TranslationColumn
                ),
            ]
        );
    }

    #[test]
    fn the_mat4_rule_scales_exactly_the_translation_column() {
        let scaled: Vec<usize> = (0..16)
            .filter(|&component| AccessorRule::Mat4TranslationColumn.scales_component(component))
            .collect();
        assert_eq!(scaled, vec![12, 13, 14]);
        let scaled: Vec<usize> = (0..16)
            .filter(|&component| JsonArrayRule::Mat4TranslationColumn.scales_component(component))
            .collect();
        assert_eq!(scaled, vec![12, 13, 14]);
        assert!((0..16).all(|c| AccessorRule::AllComponents.scales_component(c)));
    }

    #[test]
    fn the_empty_registry_rejects_every_camera_and_light_length_field() {
        assert!(LENGTH_FIELD_HANDLERS.is_empty());
        let value = json!({
            "cameras": [
                { "type": "perspective", "perspective": { "yfov": 1.0, "znear": 0.1, "zfar": 100.0, "aspectRatio": 1.5 } },
                { "type": "orthographic", "orthographic": { "xmag": 1.0, "ymag": 1.0, "znear": 0.1, "zfar": 100.0 } }
            ],
            "extensions": { "KHR_lights_punctual": { "lights": [
                { "type": "point", "range": 4.0, "intensity": 100.0, "color": [1, 1, 1] }
            ] } }
        });
        assert_eq!(
            unhandled_length_fields(&value),
            vec![
                "/cameras/0/perspective/zfar",
                "/cameras/0/perspective/znear",
                "/cameras/1/orthographic/xmag",
                "/cameras/1/orthographic/ymag",
                "/cameras/1/orthographic/zfar",
                "/cameras/1/orthographic/znear",
                "/extensions/KHR_lights_punctual/lights/0/range",
            ]
        );
    }

    #[test]
    fn an_ordinary_document_has_no_unhandled_length_field() {
        let value = json!({
            "asset": { "version": "2.0" },
            "nodes": [{ "translation": [1, 2, 3], "scale": [1, 1, 1] }],
            "materials": [{ "alphaCutoff": 0.5, "normalTexture": { "index": 0, "scale": 2.0 } }]
        });
        assert!(unhandled_length_fields(&value).is_empty());
    }

    #[test]
    fn the_two_decision_tables_are_disjoint_and_documented() {
        for field in DIMENSIONLESS_FIELDS {
            assert!(
                !field.reason.is_empty(),
                "{}/{} records no reason",
                field.owner,
                field.member
            );
            assert!(
                !UNHANDLED_LENGTH_FIELDS
                    .iter()
                    .any(|other| other.owner == field.owner && other.member == field.member),
                "{}/{} is recorded both dimensionless and length-bearing",
                field.owner,
                field.member
            );
        }
    }

    #[test]
    fn f32_component_counts_have_no_matrix_column_padding() {
        for (spelling, expected) in [
            ("SCALAR", 1),
            ("VEC2", 2),
            ("VEC3", 3),
            ("VEC4", 4),
            ("MAT2", 4),
            ("MAT3", 9),
            ("MAT4", 16),
        ] {
            assert_eq!(components_per_element(spelling), Some(expected));
        }
        assert_eq!(components_per_element("VEC5"), None);
    }
}
