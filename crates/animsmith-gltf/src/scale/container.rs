//! `.gltf` / `.glb` reassembly from a rewritten JSON tree and rewritten
//! resolved buffers.
//!
//! This never routes through [`crate::write`]: that path rebuilds a
//! normalized [`animsmith_core::Document`] and would drop every source
//! payload the conversion is supposed to preserve. Reassembly here copies the
//! source's own JSON tree and its own buffer bytes, with only the converted
//! locations changed.
//!
//! Because the rewrite is an in-place `f32` mapping, no byte length ever
//! changes: `/buffers/*/byteLength`, every `bufferViews` offset and length,
//! and every accessor `count`/`byteOffset` stay exactly as authored.

use super::GltfScaleRewriteError;
use crate::capability::{GltfBufferSourceKind, GltfCapabilityManifest, GltfContainerKind};
use crate::write::{glb_len_u32, plan_glb_lengths};
use crate::{LoadError, WriteError};
use base64::Engine as _;
use serde_json::Value;
use std::collections::BTreeSet;

const GLB_VERSION: u32 = 2;
const GLB_JSON_CHUNK: u32 = 0x4e4f_534a;
const GLB_BIN_CHUNK: u32 = 0x004e_4942;

/// Reassemble the container `manifest.container` names from `root` and
/// `buffers`.
///
/// `modified` names the buffers whose bytes changed; an untouched data-URI
/// buffer keeps its authored URI string verbatim rather than being re-encoded.
pub(crate) fn assemble(
    manifest: &GltfCapabilityManifest,
    root: &Value,
    buffers: &[Vec<u8>],
    modified: &BTreeSet<usize>,
) -> Result<Vec<u8>, GltfScaleRewriteError> {
    let mut root = root.clone();
    let binary_chunk = binary_chunk_buffer(manifest)?;
    for &buffer_index in modified {
        if Some(buffer_index) == binary_chunk {
            continue;
        }
        let payload = buffers
            .get(buffer_index)
            .ok_or_else(|| unresolved_buffer(buffer_index))?;
        reencode_data_uri(&mut root, buffer_index, payload)?;
    }
    let json = serde_json::to_vec(&root).map_err(WriteError::Serialize)?;
    match manifest.container {
        GltfContainerKind::Gltf => Ok(json),
        GltfContainerKind::Glb => {
            let bin = match binary_chunk {
                Some(index) => buffers
                    .get(index)
                    .ok_or_else(|| unresolved_buffer(index))?
                    .clone(),
                None => Vec::new(),
            };
            frame_glb(json, bin)
        }
    }
}

/// The buffer index backed by the GLB BIN chunk, if any.
///
/// glTF 2.0 requires the GLB-stored buffer to be buffer `0` and to be the
/// only one. A container declaring anything else cannot be reassembled
/// without inventing a mapping, so it fails closed.
fn binary_chunk_buffer(
    manifest: &GltfCapabilityManifest,
) -> Result<Option<usize>, GltfScaleRewriteError> {
    let mut chunk_buffers = manifest
        .buffers
        .iter()
        .filter(|buffer| buffer.source_kind == GltfBufferSourceKind::BinaryChunk)
        .map(|buffer| buffer.buffer_index);
    let first = chunk_buffers.next();
    if chunk_buffers.next().is_some() {
        return Err(GltfScaleRewriteError::UnreassemblableContainer {
            reason: "more than one buffer is backed by the GLB BIN chunk",
        });
    }
    if first.is_some_and(|index| index != 0) {
        return Err(GltfScaleRewriteError::UnreassemblableContainer {
            reason: "the GLB BIN chunk backs a buffer other than buffer 0",
        });
    }
    Ok(first)
}

/// Re-encode one buffer's base64 payload, preserving the authored URI prefix
/// up to and including `base64,` (media type, charset, and any other
/// parameters the source declared).
fn reencode_data_uri(
    root: &mut Value,
    buffer_index: usize,
    payload: &[u8],
) -> Result<(), GltfScaleRewriteError> {
    let location = format!("/buffers/{buffer_index}/uri");
    let uri = root
        .get("buffers")
        .and_then(Value::as_array)
        .and_then(|buffers| buffers.get(buffer_index))
        .and_then(|buffer| buffer.get("uri"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            LoadError::Buffer(format!(
                "{location} is not a data URI that can be rewritten"
            ))
        })?;
    let prefix_len = uri
        .find("base64,")
        .map(|index| index + "base64,".len())
        .ok_or_else(|| LoadError::Buffer(format!("{location} is not a base64 data URI")))?;
    let rewritten = format!(
        "{}{}",
        &uri[..prefix_len],
        base64::engine::general_purpose::STANDARD.encode(payload)
    );
    root["buffers"][buffer_index]["uri"] = Value::String(rewritten);
    Ok(())
}

/// Frame a GLB from its JSON and BIN payloads, padding the JSON chunk with
/// spaces and the BIN chunk with zeros to the 4-byte alignment the format
/// requires.
fn frame_glb(mut json: Vec<u8>, mut bin: Vec<u8>) -> Result<Vec<u8>, GltfScaleRewriteError> {
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let lengths = plan_glb_lengths(json.len(), bin.len())?;
    let mut out = Vec::with_capacity(lengths.total as usize);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&GLB_VERSION.to_le_bytes());
    out.extend_from_slice(&lengths.total.to_le_bytes());
    out.extend_from_slice(&lengths.json.to_le_bytes());
    out.extend_from_slice(&GLB_JSON_CHUNK.to_le_bytes());
    out.extend_from_slice(&json);
    if let Some(bin_len) = lengths.bin {
        out.extend_from_slice(&bin_len.to_le_bytes());
        out.extend_from_slice(&GLB_BIN_CHUNK.to_le_bytes());
        out.extend_from_slice(&bin);
    }
    // Re-derive the total from what was actually emitted rather than trusting
    // the plan, so a framing mistake is a typed error instead of a container
    // whose header disagrees with its own bytes.
    if glb_len_u32("total GLB length", out.len())? != lengths.total {
        return Err(GltfScaleRewriteError::UnreassemblableContainer {
            reason: "emitted GLB length disagrees with the planned header length",
        });
    }
    Ok(out)
}

fn unresolved_buffer(buffer_index: usize) -> GltfScaleRewriteError {
    LoadError::Buffer(format!("buffer {buffer_index} was not resolved")).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::GltfBufferCapability;
    use serde_json::json;

    fn manifest(
        container: GltfContainerKind,
        kinds: &[GltfBufferSourceKind],
    ) -> GltfCapabilityManifest {
        GltfCapabilityManifest {
            container,
            buffers: kinds
                .iter()
                .enumerate()
                .map(|(buffer_index, &source_kind)| GltfBufferCapability {
                    buffer_index,
                    source_kind,
                    declared_byte_length: 4,
                })
                .collect(),
            buffer_views: Vec::new(),
            accessors: Vec::new(),
            nodes: Vec::new(),
            animation_channels: Vec::new(),
            primitives: Vec::new(),
            instancing: Vec::new(),
            skins: Vec::new(),
            camera_count: 0,
            extensions: Vec::new(),
            extension_locations: Vec::new(),
            external_resource_locations: Vec::new(),
            extras_locations: Vec::new(),
            unknown_member_locations: Vec::new(),
        }
    }

    #[test]
    fn a_rewritten_data_uri_keeps_its_authored_prefix() {
        let manifest = manifest(GltfContainerKind::Gltf, &[GltfBufferSourceKind::DataUri]);
        let root = json!({
            "buffers": [{ "uri": "data:model/gltf-buffer;charset=utf-8;base64,AAAAAA==", "byteLength": 4 }]
        });
        let out = assemble(
            &manifest,
            &root,
            &[vec![1, 2, 3, 4]],
            &BTreeSet::from([0usize]),
        )
        .expect("gltf reassembly");
        let value: Value = serde_json::from_slice(&out).expect("valid JSON");
        assert_eq!(
            value["buffers"][0]["uri"],
            json!("data:model/gltf-buffer;charset=utf-8;base64,AQIDBA==")
        );
        assert_eq!(value["buffers"][0]["byteLength"], json!(4));
    }

    #[test]
    fn an_untouched_data_uri_is_copied_verbatim() {
        let manifest = manifest(GltfContainerKind::Gltf, &[GltfBufferSourceKind::DataUri]);
        // A payload the canonical encoder would not produce for these bytes:
        // proof that an unmodified buffer is never round-tripped.
        let root = json!({
            "buffers": [{ "uri": "data:application/octet-stream;base64,AAAAAA==", "byteLength": 4 }]
        });
        let out = assemble(&manifest, &root, &[vec![9, 9, 9, 9]], &BTreeSet::new())
            .expect("gltf reassembly");
        let value: Value = serde_json::from_slice(&out).expect("valid JSON");
        assert_eq!(
            value["buffers"][0]["uri"],
            json!("data:application/octet-stream;base64,AAAAAA==")
        );
    }

    #[test]
    fn glb_framing_pads_both_chunks_and_agrees_with_its_own_header() {
        let manifest = manifest(GltfContainerKind::Glb, &[GltfBufferSourceKind::BinaryChunk]);
        let root = json!({ "buffers": [{ "byteLength": 5 }] });
        let out = assemble(
            &manifest,
            &root,
            &[vec![1, 2, 3, 4, 5]],
            &BTreeSet::from([0usize]),
        )
        .expect("glb reassembly");
        assert_eq!(&out[0..4], b"glTF");
        assert_eq!(u32::from_le_bytes(out[4..8].try_into().unwrap()), 2);
        assert_eq!(
            u32::from_le_bytes(out[8..12].try_into().unwrap()) as usize,
            out.len()
        );
        let json_len = u32::from_le_bytes(out[12..16].try_into().unwrap()) as usize;
        assert!(json_len.is_multiple_of(4));
        assert_eq!(
            u32::from_le_bytes(out[16..20].try_into().unwrap()),
            GLB_JSON_CHUNK
        );
        assert_eq!(&out[20 + json_len - 1..20 + json_len], b" ");
        let bin_start = 20 + json_len;
        let bin_len =
            u32::from_le_bytes(out[bin_start..bin_start + 4].try_into().unwrap()) as usize;
        assert_eq!(bin_len, 8, "5 payload bytes pad to 8");
        assert_eq!(
            u32::from_le_bytes(out[bin_start + 4..bin_start + 8].try_into().unwrap()),
            GLB_BIN_CHUNK
        );
        assert_eq!(
            &out[bin_start + 8..bin_start + 8 + bin_len],
            &[1, 2, 3, 4, 5, 0, 0, 0]
        );
        assert_eq!(out.len(), 12 + 8 + json_len + 8 + bin_len);
    }

    #[test]
    fn a_glb_binary_chunk_outside_buffer_zero_is_refused() {
        let manifest = manifest(
            GltfContainerKind::Glb,
            &[
                GltfBufferSourceKind::DataUri,
                GltfBufferSourceKind::BinaryChunk,
            ],
        );
        let root = json!({ "buffers": [{ "uri": "data:x;base64,AAAAAA==" }, { "byteLength": 4 }] });
        let error = assemble(
            &manifest,
            &root,
            &[vec![0; 4], vec![0; 4]],
            &BTreeSet::new(),
        )
        .expect_err("BIN chunk must back buffer 0");
        assert!(matches!(
            error,
            GltfScaleRewriteError::UnreassemblableContainer {
                reason: "the GLB BIN chunk backs a buffer other than buffer 0"
            }
        ));
    }

    #[test]
    fn two_binary_chunk_buffers_are_refused() {
        let manifest = manifest(
            GltfContainerKind::Glb,
            &[
                GltfBufferSourceKind::BinaryChunk,
                GltfBufferSourceKind::BinaryChunk,
            ],
        );
        let root = json!({ "buffers": [{ "byteLength": 4 }, { "byteLength": 4 }] });
        let error = assemble(
            &manifest,
            &root,
            &[vec![0; 4], vec![0; 4]],
            &BTreeSet::new(),
        )
        .expect_err("two BIN-backed buffers are ambiguous");
        assert!(matches!(
            error,
            GltfScaleRewriteError::UnreassemblableContainer {
                reason: "more than one buffer is backed by the GLB BIN chunk"
            }
        ));
    }
}
