//! Repair-specific ownership of original source storage. Range arithmetic and
//! overlap scanning stay shared with capability inspection; scale's acceptance
//! rules and dimensionless classification cannot authorize quaternion writes.

use crate::capability::{
    OwnedRange, RangeOwner, as_index, declared_view_range, image_payload_ranges,
    json_pointer_child, json_pointer_token, overlapping_ranges, preserved_accessor_ranges,
    raw_json_bytes,
};
use crate::{LoadError, safe_external_buffer_path};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub(super) enum Ownership {
    Known(Vec<Option<String>>),
    Unavailable(String),
}

impl Ownership {
    pub(super) fn read(original: &[u8], buffers: &[Vec<u8>]) -> Result<Self, LoadError> {
        let (_, json) = raw_json_bytes(original)?;
        // Typed glTF parsing can ignore deeply nested metadata that the raw
        // inventory cannot model within its parser limit. Leave it untouched.
        let root: Value = match serde_json::from_slice(json) {
            Ok(root) => root,
            Err(error) => {
                return Ok(Self::Unavailable(format!(
                    "ownership unavailable: source inventory could not be parsed: {error}"
                )));
            }
        };
        Ok(match inspect(&root, buffers) {
            Ok(reasons) => Self::Known(reasons),
            Err(reason) => Self::Unavailable(format!("ownership unavailable: {reason}")),
        })
    }

    pub(super) fn reason(&self, accessor: usize) -> Option<&str> {
        match self {
            Self::Known(reasons) => reasons.get(accessor).map_or(
                Some("ownership unavailable: accessor missing from original source inventory"),
                Option::as_deref,
            ),
            Self::Unavailable(reason) => Some(reason),
        }
    }
}

fn inspect(root: &Value, buffers: &[Vec<u8>]) -> Result<Vec<Option<String>>, String> {
    let object = root.as_object().ok_or("source root is not an object")?;
    // The writer publishes one Vec per declared buffer. Even disjoint ranges
    // cannot authorize writes when a later alias can overwrite that Vec.
    let mut external_keys = BTreeMap::new();
    for (index, buffer) in array(root, "buffers").iter().enumerate() {
        if let Some(uri) = buffer.get("uri").and_then(Value::as_str)
            && !uri.starts_with("data:")
        {
            let key = safe_external_buffer_path(uri).map_err(|error| error.to_string())?;
            if let Some(previous) = external_keys.insert(key, index) {
                return Err(format!(
                    "/buffers/{index}/uri aliases /buffers/{previous}/uri"
                ));
            }
        }
    }
    let accessors = array(root, "accessors");
    let uses = accessor_uses(root, accessors.len())?;
    let mut ranges = Vec::new();
    for (index, accessor) in accessors.iter().enumerate() {
        let mut owned = preserved_accessor_ranges(object, buffers, index)
            .ok_or_else(|| format!("/accessors/{index} has unresolved storage"))?;
        for range in &mut owned {
            range.4 = matches!(range.3, RangeOwner::Accessor(_))
                && matches!(uses[index], AccessorUse::Rotation(_))
                && accessor.get("sparse").is_none();
        }
        ranges.extend(owned);
    }
    ranges.extend(image_payload_ranges(object));
    for declaration in ["extensionsUsed", "extensionsRequired"] {
        for (index, extension) in array(root, declaration).iter().enumerate() {
            let name = extension
                .as_str()
                .ok_or_else(|| format!("/{declaration}/{index}"))?;
            if !known_extension(name) {
                return Err(format!(
                    "/{declaration}/{index} declares unmodeled extension {name}"
                ));
            }
        }
    }
    extension_ranges(root, &mut Vec::new(), object, &mut ranges)?;
    let overlaps = overlapping_ranges(&mut ranges);
    Ok(uses
        .into_iter()
        .enumerate()
        .map(|(index, usage)| {
            if let AccessorUse::Preserved(location) = usage {
                Some(format!("also used by {location}"))
            } else {
                overlaps
                    .get(&RangeOwner::Accessor(index))
                    .map(|owner| format!("storage overlaps {}", owner.location()))
            }
        })
        .collect())
}

#[derive(Clone)]
enum AccessorUse {
    Unused,
    Rotation(bool), // cubic triplets versus LINEAR/STEP values
    Preserved(String),
}

fn record_use(
    uses: &mut [AccessorUse],
    index: Option<&Value>,
    rotation: Option<bool>,
    location: String,
) -> Result<(), String> {
    let Some(index) = index else { return Ok(()) };
    let usage = as_index(Some(index))
        .and_then(|index| uses.get_mut(index))
        .ok_or_else(|| format!("{location} has an unresolved accessor reference"))?;
    match (&*usage, rotation) {
        (AccessorUse::Preserved(_), _) => {}
        (_, None) => *usage = AccessorUse::Preserved(location),
        (AccessorUse::Rotation(previous), Some(cubic)) if *previous != cubic => {
            *usage = AccessorUse::Preserved(format!(
                "{location} with incompatible rotation interpolation"
            ));
        }
        (_, Some(cubic)) => *usage = AccessorUse::Rotation(cubic),
    }
    Ok(())
}

fn accessor_uses(root: &Value, count: usize) -> Result<Vec<AccessorUse>, String> {
    let mut uses = vec![AccessorUse::Unused; count];
    for (mesh, object) in array(root, "meshes").iter().enumerate() {
        for (primitive, object) in array(object, "primitives").iter().enumerate() {
            let location = format!("/meshes/{mesh}/primitives/{primitive}");
            if let Some(attributes) = object.get("attributes").and_then(Value::as_object) {
                for (semantic, index) in attributes {
                    record_use(
                        &mut uses,
                        Some(index),
                        None,
                        json_pointer_child(&format!("{location}/attributes"), semantic),
                    )?;
                }
            }
            record_use(
                &mut uses,
                object.get("indices"),
                None,
                format!("{location}/indices"),
            )?;
            for (target, attributes) in array(object, "targets").iter().enumerate() {
                if let Some(attributes) = attributes.as_object() {
                    for (semantic, index) in attributes {
                        record_use(
                            &mut uses,
                            Some(index),
                            None,
                            json_pointer_child(&format!("{location}/targets/{target}"), semantic),
                        )?;
                    }
                }
            }
        }
    }
    for (skin, object) in array(root, "skins").iter().enumerate() {
        record_use(
            &mut uses,
            object.get("inverseBindMatrices"),
            None,
            format!("/skins/{skin}/inverseBindMatrices"),
        )?;
    }
    for (animation, object) in array(root, "animations").iter().enumerate() {
        let samplers = array(object, "samplers");
        let mut referenced = vec![false; samplers.len()];
        for (channel, object) in array(object, "channels").iter().enumerate() {
            let location = format!("/animations/{animation}/channels/{channel}");
            let sampler = as_index(object.get("sampler"))
                .filter(|&index| index < samplers.len())
                .ok_or_else(|| format!("{location}/sampler is unresolved"))?;
            referenced[sampler] = true;
            let output = &samplers[sampler];
            let rotation = (object.pointer("/target/path").and_then(Value::as_str)
                == Some("rotation"))
            .then(|| output.get("interpolation").and_then(Value::as_str) == Some("CUBICSPLINE"));
            record_use(&mut uses, output.get("output"), rotation, location)?;
        }
        for (sampler, object) in samplers.iter().enumerate() {
            let location = format!("/animations/{animation}/samplers/{sampler}");
            record_use(
                &mut uses,
                object.get("input"),
                None,
                format!("{location}/input"),
            )?;
            if !referenced[sampler] {
                record_use(
                    &mut uses,
                    object.get("output"),
                    None,
                    format!("{location}/output (unreferenced sampler)"),
                )?;
            }
        }
    }
    Ok(uses)
}

fn array<'a>(object: &'a Value, member: &str) -> &'a [Value] {
    object
        .get(member)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

/// These extensions add JSON values or references to already-inventoried core
/// images/materials/nodes, not additional accessor or buffer-view owners.
/// Keep names explicit: a future material/texture extension may own storage.
fn known_extension(name: &str) -> bool {
    matches!(
        name,
        "KHR_materials_unlit"
            | "KHR_materials_pbrSpecularGlossiness"
            | "KHR_materials_clearcoat"
            | "KHR_materials_transmission"
            | "KHR_materials_volume"
            | "KHR_materials_ior"
            | "KHR_materials_sheen"
            | "KHR_materials_specular"
            | "KHR_materials_iridescence"
            | "KHR_materials_anisotropy"
            | "KHR_materials_emissive_strength"
            | "KHR_materials_dispersion"
            | "KHR_materials_variants"
            | "KHR_texture_transform"
            | "KHR_texture_basisu"
            | "EXT_texture_webp"
            | "EXT_texture_avif"
            | "KHR_lights_punctual"
            | "KHR_mesh_quantization"
            | "MSFT_lod"
            | "KHR_draco_mesh_compression"
    )
}

enum PathSegment<'a> {
    Key(&'a str),
    Index(usize),
}

// Only error paths need rendered pointers. Borrow keys during traversal so a
// wide array beneath a long metadata key does not repeatedly copy that key.
fn source_pointer(path: &[PathSegment<'_>]) -> String {
    let mut pointer = String::new();
    for segment in path {
        pointer.push('/');
        match segment {
            PathSegment::Key(key) => pointer.push_str(&json_pointer_token(key)),
            PathSegment::Index(index) => pointer.push_str(&index.to_string()),
        }
    }
    pointer
}

fn extension_ranges<'a>(
    value: &'a Value,
    path: &mut Vec<PathSegment<'a>>,
    root: &Map<String, Value>,
    ranges: &mut Vec<OwnedRange>,
) -> Result<(), String> {
    match value {
        Value::Object(object) => {
            if let Some(extensions) = object.get("extensions").and_then(Value::as_object) {
                path.push(PathSegment::Key("extensions"));
                for (name, payload) in extensions {
                    path.push(PathSegment::Key(name));
                    if !known_extension(name) {
                        return Err(format!(
                            "{} has unmodeled payload ownership",
                            source_pointer(path)
                        ));
                    }
                    if name == "KHR_draco_mesh_compression" {
                        let unresolved =
                            || format!("{}/bufferView is unresolved", source_pointer(path));
                        let view = as_index(payload.get("bufferView")).ok_or_else(unresolved)?;
                        let (buffer, start, end) =
                            declared_view_range(root, view).ok_or_else(unresolved)?;
                        ranges.push((buffer, start, end, RangeOwner::ExtensionView(view), false));
                    }
                    path.pop();
                }
                path.pop();
            }
            for (key, child) in object {
                path.push(PathSegment::Key(key));
                extension_ranges(child, path, root, ranges)?;
                path.pop();
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                path.push(PathSegment::Index(index));
                extension_ranges(child, path, root, ranges)?;
                path.pop();
            }
        }
        _ => {}
    }
    Ok(())
}
