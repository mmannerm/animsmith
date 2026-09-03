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
//! the GLB's JSON and BIN chunks and digests them with exactly one
//! normalization: the `asset.generator` string is replaced by a fixed
//! placeholder, and the JSON chunk's trailing padding is dropped so a
//! longer or shorter version string cannot move the digest. Nothing inside
//! those two chunks is normalized otherwise — key order, number
//! formatting, whitespace and every BIN byte are digested as they are on
//! disk.
//!
//! [`restamped`] is the other half: the same GLB with another release's
//! stamp, re-framed so the stamp may be any length. [`bin_chunk`] hands
//! back the payload bytes themselves, for a comparison that wants bytes
//! rather than a digest.

use animsmith_core::sha256_hex;
use serde_json::Value;

/// GLB container magic (`glTF`).
const GLB_MAGIC: &[u8; 4] = b"glTF";
/// Chunk type of the JSON chunk.
const JSON_CHUNK: &[u8; 4] = b"JSON";
/// Chunk type of the binary buffer chunk.
const BIN_CHUNK: &[u8; 4] = b"BIN\0";
/// Stands in for the serialized `asset.generator` string while the JSON
/// chunk is digested. Its length is fixed, so two releases whose version
/// strings differ in length still digest to the same bytes.
const GENERATOR_PLACEHOLDER: &[u8] = b"\"<generator>\"";

/// A GLB's payload identity: what its two chunks hold, minus the release
/// stamp. Produced by [`payload_identity`], which documents both digests.
#[derive(Debug)]
pub struct GlbPayloadIdentity {
    /// SHA-256 of the JSON chunk with its trailing padding removed and the
    /// serialized `asset.generator` string replaced by a fixed placeholder.
    pub json_sha256: String,
    /// SHA-256 of the BIN chunk's bytes verbatim, including the zero
    /// padding that the chunk length covers. A GLB with no BIN chunk
    /// digests as the empty byte string.
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
/// both digests, while a difference inside either chunk — a keyframe
/// value, a node name, an accessor count — changes one of them. The
/// identity covers the first JSON chunk and the first BIN chunk only: the
/// container's own framing, and any further chunk, is not digested.
///
/// # Errors
///
/// Returns a description of the first thing that does not hold: the GLB
/// header or its chunk framing does not parse, there is no JSON chunk, the
/// JSON chunk is not valid JSON, or its `asset.generator` string does not
/// appear in the chunk exactly once in `serde_json`'s serialization (the
/// form every writer in this workspace emits). A JSON chunk carrying no
/// `asset.generator` string is read normally, with `generator` reported as
/// `None`.
pub fn payload_identity(bytes: &[u8]) -> Result<GlbPayloadIdentity, String> {
    let glb = Glb::read(bytes)?;
    let json = match &glb.generator {
        Some(generator) => replace_generator(glb.json, generator, GENERATOR_PLACEHOLDER)?,
        None => glb.json.to_vec(),
    };
    Ok(GlbPayloadIdentity {
        json_sha256: sha256_hex(&json),
        bin_sha256: sha256_hex(glb.bin.unwrap_or_default()),
        generator: glb.generator,
    })
}

/// The same GLB with `generator` in `asset.generator` instead of the stamp
/// it carries.
///
/// The container is re-framed — the JSON chunk's padding and every length
/// field are recomputed — so the new stamp may be any length. That is how
/// a test simulates another release's output from a committed file: a real
/// bump changes the stamp's length whenever a version part gains a digit.
///
/// # Errors
///
/// The conditions [`payload_identity`] lists, plus a JSON chunk that
/// carries no `asset.generator` string to replace.
pub fn restamped(bytes: &[u8], generator: &str) -> Result<Vec<u8>, String> {
    let glb = Glb::read(bytes)?;
    let stamp = glb
        .generator
        .as_deref()
        .ok_or_else(|| "GLB carries no asset.generator to restamp".to_owned())?;
    let replacement = serialize(generator)?;
    let json = replace_generator(glb.json, stamp, replacement.as_bytes())?;
    Ok(frame(&json, glb.bin))
}

/// The GLB's BIN chunk payload, empty when the container omits the chunk.
///
/// These are the bytes [`payload_identity`] digests, verbatim and
/// including the zero padding the chunk length covers, for a comparison
/// that holds two files' payloads against each other byte for byte rather
/// than by digest.
///
/// # Errors
///
/// The container errors [`payload_identity`] lists.
pub fn bin_chunk(bytes: &[u8]) -> Result<&[u8], String> {
    Ok(Glb::read(bytes)?.bin.unwrap_or_default())
}

/// A GLB read far enough to talk about its payload.
struct Glb<'a> {
    /// JSON chunk with its trailing padding trimmed.
    json: &'a [u8],
    /// BIN chunk payload, `None` when the container omits the chunk (which
    /// a GLB with no binary data must, an empty chunk being invalid).
    bin: Option<&'a [u8]>,
    /// The `asset.generator` string the JSON chunk carries, if any.
    generator: Option<String>,
}

impl<'a> Glb<'a> {
    fn read(bytes: &'a [u8]) -> Result<Self, String> {
        let (json, bin) = chunks(bytes)?;
        let json = trim_padding(json);
        let root: Value = serde_json::from_slice(json)
            .map_err(|error| format!("JSON chunk does not parse: {error}"))?;
        let generator = root
            .get("asset")
            .and_then(|asset| asset.get("generator"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        Ok(Self {
            json,
            bin,
            generator,
        })
    }
}

/// The JSON and BIN chunk payloads of a GLB, the BIN one `None` when the
/// container omits it. Chunks of any other type are skipped, as the
/// specification requires of a reader, and so is a repeat of either type.
fn chunks(bytes: &[u8]) -> Result<(&[u8], Option<&[u8]>), String> {
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
        bin,
    ))
}

/// A GLB container around `json` and `bin`, padded and length-stamped as
/// the specification requires. The BIN chunk is omitted for `None` rather
/// than emitted empty, which is invalid.
fn frame(json: &[u8], bin: Option<&[u8]>) -> Vec<u8> {
    let mut json = json.to_vec();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let bin = bin.map(|bin| {
        let mut bin = bin.to_vec();
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
        bin
    });
    let total = 12 + 8 + json.len() + bin.as_ref().map_or(0, |bin| 8 + bin.len());
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(GLB_MAGIC);
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(JSON_CHUNK);
    out.extend_from_slice(&json);
    if let Some(bin) = bin {
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(BIN_CHUNK);
        out.extend_from_slice(&bin);
    }
    out
}

/// `json` with the serialized form of the `stamp` string replaced by
/// `replacement`.
///
/// The stamp is located as `serde_json` would write it — quoted and
/// escaped — and must appear exactly once, so a second copy of the same
/// string elsewhere in the chunk is refused rather than guessed at.
fn replace_generator(json: &[u8], stamp: &str, replacement: &[u8]) -> Result<Vec<u8>, String> {
    let needle = serialize(stamp)?;
    replace_only_occurrence(json, needle.as_bytes(), replacement)
        .ok_or_else(|| format!("JSON chunk does not carry {needle} exactly once"))
}

/// A string as JSON: quoted and escaped the way the writer emitted it.
fn serialize(value: &str) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| format!("{value:?} does not serialize: {error}"))
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
/// when `needle` is empty or occurs any number of times other than once.
fn replace_only_occurrence(haystack: &[u8], needle: &[u8], replacement: &[u8]) -> Option<Vec<u8>> {
    if needle.is_empty() {
        return None;
    }
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
    let mut out = haystack.to_vec();
    out.splice(start..start + needle.len(), replacement.iter().copied());
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glb_container;
    use std::collections::BTreeSet;

    /// The fixture's JSON chunk: `asset` fields, then one named node.
    fn rig(asset_fields: &str, node: &str) -> String {
        format!(r#"{{"asset":{{{asset_fields}"version":"2.0"}},"nodes":[{{"name":"{node}"}}]}}"#)
    }

    /// A minimal GLB carrying one named node and `bin` as its buffer,
    /// framed by the shared fixture builder rather than by this module.
    fn glb(generator: &str, node: &str, bin: &[u8]) -> Vec<u8> {
        let asset_fields = format!(r#""generator":"{generator}","#);
        glb_container(rig(&asset_fields, node).as_bytes(), bin)
    }

    /// A GLB with a JSON chunk and no BIN chunk, framed by hand — what the
    /// writer emits for a document with no binary data.
    fn json_only_glb(json: &str) -> Vec<u8> {
        let mut json = json.as_bytes().to_vec();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let total = 12 + 8 + json.len();
        let mut out = b"glTF".to_vec();
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&json);
        out
    }

    fn identity(bytes: &[u8]) -> GlbPayloadIdentity {
        payload_identity(bytes).expect("the fixture is a readable GLB")
    }

    #[test]
    fn every_release_stamp_of_one_payload_shares_an_identity() {
        // Four stamps of four different lengths, the way 0.9.9 → 0.10.0
        // lengthened the string in every committed asset. A longer stamp
        // moves the JSON chunk length, its four-byte padding, the
        // container's total-length field and the offset the BIN chunk
        // starts at, so none of those may reach the identity.
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
        for ((stamp, identity), file) in stamps.iter().zip(&identities).zip(&files) {
            assert_eq!(identity.generator.as_deref(), Some(*stamp));
            assert_eq!(
                identity.json_sha256, identities[0].json_sha256,
                "the release stamp is the only JSON difference"
            );
            assert_eq!(
                bin_chunk(file).expect("the fixture is a readable GLB"),
                &[1, 2, 3, 4],
                "the buffer must come back byte for byte, not merely as a matching digest"
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
    fn a_glb_without_a_generator_still_has_an_identity_but_cannot_be_restamped() {
        let stampless = glb_container(rig("", "root").as_bytes(), &[1, 2, 3, 4]);
        let identity = identity(&stampless);

        assert_eq!(identity.generator, None);
        assert_eq!(
            identity.bin_sha256,
            sha256_hex(&[1, 2, 3, 4]),
            "the BIN chunk is digested verbatim"
        );
        assert_eq!(
            restamped(&stampless, "animsmith 0.11.0").expect_err("there is no stamp to replace"),
            "GLB carries no asset.generator to restamp"
        );
    }

    #[test]
    fn a_glb_without_a_bin_chunk_digests_as_an_empty_payload() {
        // The writer omits the BIN chunk for a document with no binary
        // data rather than emitting an empty one, which GLB forbids.
        let bufferless = json_only_glb(&rig(r#""generator":"animsmith 0.10.0","#, "root"));

        assert_eq!(bin_chunk(&bufferless).expect("readable GLB"), b"");
        assert_eq!(
            identity(&bufferless).bin_sha256,
            sha256_hex(b""),
            "an absent BIN chunk digests as the empty byte string"
        );

        let restamped = restamped(&bufferless, "animsmith 0.100.111").expect("restamps");
        assert!(
            restamped.len() > bufferless.len(),
            "the longer stamp must grow the container"
        );
        assert_eq!(
            identity(&restamped).json_sha256,
            identity(&bufferless).json_sha256,
            "re-framing a bufferless GLB keeps its payload identity"
        );
    }

    #[test]
    fn restamping_replaces_the_stamp_and_nothing_else() {
        let before = glb("animsmith 0.10.0", "root", &[1, 2, 3, 4]);
        let after = restamped(&before, "animsmith 0.100.111").expect("restamps");

        assert_eq!(
            identity(&after).generator.as_deref(),
            Some("animsmith 0.100.111")
        );
        assert_ne!(
            after.len(),
            before.len(),
            "a stamp three bytes longer must re-frame the container"
        );
        assert_eq!(
            bin_chunk(&after).expect("readable GLB"),
            bin_chunk(&before).expect("readable GLB"),
            "the BIN chunk must survive a restamp byte for byte"
        );
        assert_eq!(
            identity(&after).json_sha256,
            identity(&before).json_sha256,
            "and the JSON chunk must differ only in the stamp"
        );
    }

    #[test]
    fn a_second_copy_of_the_stamp_is_refused_rather_than_guessed_at() {
        // The node is named after the stamp, so the serialized string
        // occurs twice and nothing can tell which one is the provenance.
        let ambiguous = glb("animsmith 0.10.0", "animsmith 0.10.0", &[1, 2, 3, 4]);

        let error = payload_identity(&ambiguous).expect_err("the stamp is ambiguous");
        assert!(
            error.contains("exactly once"),
            "the error must name the ambiguity: {error}"
        );
        assert!(
            restamped(&ambiguous, "animsmith 0.11.0").is_err(),
            "and a restamp must not guess either"
        );
    }

    #[test]
    fn malformed_containers_are_reported_rather_than_read() {
        let good = glb("animsmith 0.10.0", "root", &[1, 2, 3, 4]);

        assert!(payload_identity(&good[..8]).is_err(), "truncated header");

        let mut wrong_magic = good.clone();
        wrong_magic[..4].copy_from_slice(b"GLTF");
        assert!(payload_identity(&wrong_magic).is_err(), "wrong magic");

        let mut wrong_version = good.clone();
        wrong_version[4..8].copy_from_slice(&1u32.to_le_bytes());
        assert!(payload_identity(&wrong_version).is_err(), "not GLB 2");

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

        let mut no_json = good.clone();
        no_json[16..20].copy_from_slice(BIN_CHUNK);
        assert!(payload_identity(&no_json).is_err(), "no JSON chunk");

        assert!(
            payload_identity(&json_only_glb(r#"{"asset": "#)).is_err(),
            "a JSON chunk that does not parse"
        );
    }
}
