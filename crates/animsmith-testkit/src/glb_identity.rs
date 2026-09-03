//! What a GLB stores, minus which release wrote it.
//!
//! Every GLB this workspace writes carries the package version in
//! `asset.generator` (`crates/animsmith-gltf/src/write.rs`), so a release
//! bump rewrites bytes inside every committed example asset — and shifts
//! the JSON chunk's padding and the container's length fields whenever the
//! new version string is a different length. Byte equality therefore
//! cannot distinguish "the release stamp moved" from "the animation
//! changed".
//!
//! [`payload_identity`] answers the second question on its own. It reads
//! the GLB's two chunks and digests them with exactly one normalization:
//! the `asset.generator` string is replaced by a fixed placeholder, and
//! the JSON chunk's trailing padding is dropped so a longer or shorter
//! version string cannot move the digest. Nothing else is normalized —
//! key order, number formatting, whitespace and every BIN byte are
//! digested as they are on disk — so the identity changes for any content
//! change and holds across release-version bumps.

use animsmith_core::sha256_hex;
use serde_json::Value;

/// GLB container magic (`glTF`), little-endian.
const GLB_MAGIC: &[u8; 4] = b"glTF";
/// Chunk type of the JSON chunk.
const JSON_CHUNK: &[u8; 4] = b"JSON";
/// Chunk type of the binary buffer chunk.
const BIN_CHUNK: &[u8; 4] = b"BIN\0";
/// Stands in for the serialized `asset.generator` string while the JSON
/// chunk is digested. Its length is fixed, so two releases whose version
/// strings differ in length still digest to the same bytes.
const GENERATOR_PLACEHOLDER: &[u8] = b"\"<generator>\"";

/// A GLB's payload identity: what it stores, minus its release stamp.
///
/// Produced by [`payload_identity`], which documents both digests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlbPayloadIdentity {
    /// SHA-256 of the JSON chunk with its trailing padding removed and the
    /// serialized `asset.generator` string replaced by a fixed placeholder.
    pub json_sha256: String,
    /// SHA-256 of the BIN chunk's bytes verbatim, including the zero
    /// padding that the chunk length covers. Empty input (a GLB with no
    /// BIN chunk) digests as the empty byte string.
    pub bin_sha256: String,
    /// The `asset.generator` string that was replaced, when the JSON chunk
    /// carries one as a string.
    pub generator: Option<String>,
}

/// The payload identity of `bytes`, a complete GLB file.
///
/// The JSON half is digested with the generator string replaced and the
/// chunk's trailing padding trimmed; the BIN half is digested verbatim.
/// Two GLBs written by different releases from the same document share
/// both digests, while any other difference — a keyframe value, a node
/// name, an accessor count — changes one of them.
///
/// # Errors
///
/// Returns a description of the first thing that does not hold: the GLB
/// header or its chunk framing does not parse, there is no JSON chunk,
/// the JSON chunk is not valid JSON, or the generator string does not
/// appear in the chunk exactly once in `serde_json`'s serialization (the
/// form every writer in this workspace emits).
pub fn payload_identity(bytes: &[u8]) -> Result<GlbPayloadIdentity, String> {
    let (json, bin) = chunks(bytes)?;
    let json = trim_padding(json);
    let root: Value = serde_json::from_slice(json)
        .map_err(|error| format!("JSON chunk does not parse: {error}"))?;
    let generator = root
        .get("asset")
        .and_then(|asset| asset.get("generator"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let digested = match &generator {
        Some(generator) => {
            let needle = serde_json::to_string(generator)
                .map_err(|error| format!("generator string does not serialize: {error}"))?;
            replace_only_occurrence(json, needle.as_bytes(), GENERATOR_PLACEHOLDER)
                .ok_or_else(|| format!("JSON chunk does not carry {needle} exactly once"))?
        }
        None => json.to_vec(),
    };
    Ok(GlbPayloadIdentity {
        json_sha256: sha256_hex(&digested),
        bin_sha256: sha256_hex(bin),
        generator,
    })
}

/// The JSON and BIN chunk payloads of a GLB. The BIN slice is empty when
/// the container carries no BIN chunk (which a GLB with no binary data
/// must omit rather than emit empty). Chunks of any other type are
/// skipped, as the specification requires of a reader.
fn chunks(bytes: &[u8]) -> Result<(&[u8], &[u8]), String> {
    let header = bytes
        .get(..12)
        .ok_or_else(|| format!("{} bytes is shorter than a GLB header", bytes.len()))?;
    if &header[..4] != GLB_MAGIC {
        return Err("not a GLB: the file does not start with the glTF magic".to_owned());
    }
    let version = u32_at(header, 4);
    if version != 2 {
        return Err(format!("GLB container version {version} is not 2"));
    }
    let total = u32_at(header, 8) as usize;
    if total != bytes.len() {
        return Err(format!(
            "GLB header claims {total} bytes, the file has {}",
            bytes.len()
        ));
    }
    let mut json = None;
    let mut bin = None;
    let mut cursor = 12;
    while cursor < bytes.len() {
        let framing = bytes
            .get(cursor..cursor + 8)
            .ok_or_else(|| format!("chunk at byte {cursor} has no complete 8-byte header"))?;
        let length = u32_at(framing, 0) as usize;
        let end = cursor
            .checked_add(8)
            .and_then(|start| start.checked_add(length))
            .ok_or_else(|| format!("chunk at byte {cursor} overflows the file length"))?;
        let data = bytes
            .get(cursor + 8..end)
            .ok_or_else(|| format!("chunk at byte {cursor} claims {length} bytes past the file"))?;
        match &framing[4..8] {
            kind if kind == JSON_CHUNK && json.is_none() => json = Some(data),
            kind if kind == BIN_CHUNK && bin.is_none() => bin = Some(data),
            _ => {}
        }
        cursor = end;
    }
    Ok((
        json.ok_or_else(|| "GLB carries no JSON chunk".to_owned())?,
        bin.unwrap_or_default(),
    ))
}

/// The little-endian `u32` at `offset` in a slice known to hold it.
fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    let mut field = [0u8; 4];
    field.copy_from_slice(&bytes[offset..offset + 4]);
    u32::from_le_bytes(field)
}

/// The JSON chunk without the trailing spaces that pad it to a four-byte
/// boundary. Compact JSON ends with `}`, and a JSON document's trailing
/// bytes can only be padding, so this removes exactly the padding.
fn trim_padding(json: &[u8]) -> &[u8] {
    let end = json
        .iter()
        .rposition(|&byte| byte != b' ')
        .map_or(0, |last| last + 1);
    &json[..end]
}

/// `haystack` with its single occurrence of `needle` replaced, or `None`
/// when `needle` occurs any number of times other than once.
///
/// [`payload_identity`] uses it to swap the release stamp out; a caller
/// that wants the inverse — the same file stamped with another release —
/// swaps it in the same way, on the same exactly-once rule.
pub fn replace_only_occurrence(
    haystack: &[u8],
    needle: &[u8],
    replacement: &[u8],
) -> Option<Vec<u8>> {
    let mut found = None;
    for (start, window) in haystack.windows(needle.len()).enumerate() {
        if window == needle {
            if found.is_some() {
                return None;
            }
            found = Some(start);
        }
    }
    let start = found?;
    let mut out = Vec::with_capacity(haystack.len() - needle.len() + replacement.len());
    out.extend_from_slice(&haystack[..start]);
    out.extend_from_slice(replacement);
    out.extend_from_slice(&haystack[start + needle.len()..]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glb_container;
    use std::collections::BTreeSet;

    /// A minimal GLB carrying one named node and `bin` as its buffer.
    fn glb(generator: &str, node: &str, bin: &[u8]) -> Vec<u8> {
        let json = format!(
            r#"{{"asset":{{"version":"2.0","generator":"{generator}"}},"nodes":[{{"name":"{node}"}}]}}"#
        );
        glb_container(json.as_bytes(), bin)
    }

    fn identity(bytes: &[u8]) -> GlbPayloadIdentity {
        payload_identity(bytes).expect("the fixture is a readable GLB")
    }

    #[test]
    fn every_release_stamp_of_one_payload_shares_an_identity() {
        // Four stamps of four different lengths, the way 0.9.9 → 0.10.0
        // lengthened the string in every committed asset. A longer stamp
        // moves the JSON chunk length, its four-byte padding and the
        // container's total-length field, so none of those may reach the
        // identity.
        let stamps = [
            "animsmith 0.9.9",
            "animsmith 0.10.0",
            "animsmith 0.10.11",
            "animsmith 0.100.111",
        ];
        let files: Vec<Vec<u8>> = stamps
            .iter()
            .map(|stamp| glb(stamp, "root", &[1, 2, 3, 4]))
            .collect();
        let lengths: BTreeSet<usize> = files.iter().map(Vec::len).collect();
        assert!(
            lengths.len() > 1,
            "the stamps must move the container framing, not only its bytes: {lengths:?}"
        );

        let identities: Vec<GlbPayloadIdentity> = files.iter().map(|glb| identity(glb)).collect();
        for (stamp, identity) in stamps.iter().zip(&identities) {
            assert_eq!(identity.generator.as_deref(), Some(*stamp));
            assert_eq!(
                identity.json_sha256, identities[0].json_sha256,
                "the release stamp is the only JSON difference"
            );
            assert_eq!(
                identity.bin_sha256, identities[0].bin_sha256,
                "the buffer is untouched"
            );
        }
    }

    #[test]
    fn one_changed_buffer_byte_changes_the_bin_half_alone() {
        let before = identity(&glb("animsmith 0.10.0", "root", &[1, 2, 3, 4]));
        let after = identity(&glb("animsmith 0.10.0", "root", &[1, 2, 3, 5]));

        assert_ne!(
            before.bin_sha256, after.bin_sha256,
            "a changed buffer byte must change the payload identity"
        );
        assert_eq!(
            before.json_sha256, after.json_sha256,
            "the JSON chunk did not change"
        );
    }

    #[test]
    fn one_changed_json_leaf_outside_the_generator_changes_the_json_half() {
        let before = identity(&glb("animsmith 0.10.0", "root", &[1, 2, 3, 4]));
        let after = identity(&glb("animsmith 0.10.0", "hips", &[1, 2, 3, 4]));

        assert_ne!(
            before.json_sha256, after.json_sha256,
            "a renamed node must change the payload identity"
        );
        assert_eq!(
            before.bin_sha256, after.bin_sha256,
            "the buffer is untouched"
        );
    }

    #[test]
    fn a_glb_without_a_generator_still_has_an_identity() {
        let json = br#"{"asset":{"version":"2.0"},"nodes":[{"name":"root"}]}"#;
        let identity = identity(&glb_container(json, &[1, 2, 3, 4]));

        assert_eq!(identity.generator, None);
        assert_eq!(
            identity.bin_sha256,
            sha256_hex(&[1, 2, 3, 4]),
            "the BIN chunk is digested verbatim"
        );
    }

    #[test]
    fn malformed_containers_are_reported_rather_than_read() {
        let good = glb("animsmith 0.10.0", "root", &[1, 2, 3, 4]);

        assert!(payload_identity(&good[..8]).is_err(), "truncated header");

        let mut wrong_magic = good.clone();
        wrong_magic[..4].copy_from_slice(b"GLTF");
        assert!(payload_identity(&wrong_magic).is_err(), "wrong magic");

        let mut short = good.clone();
        short.pop();
        assert!(
            payload_identity(&short).is_err(),
            "the header's total length no longer matches the file"
        );

        let mut lying_chunk = good.clone();
        lying_chunk[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(
            payload_identity(&lying_chunk).is_err(),
            "a chunk length past the end of the file"
        );
    }
}
