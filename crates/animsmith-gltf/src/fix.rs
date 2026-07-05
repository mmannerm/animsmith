//! Byte-surgical clip repairs: mutate only the animation accessor
//! bytes that need to change and copy everything else through
//! verbatim. A fixed character GLB keeps its meshes, skins, materials,
//! and textures byte-identical — the output diff is exactly the
//! repaired keys.
//!
//! Quaternion repairs:
//! - `quat-norm`: normalize non-unit quaternion keys. Scaling a finite
//!   non-zero quaternion back to unit length preserves the represented
//!   rotation and avoids engine-dependent renormalization.
//! - `quat-flip`: adjacent rotation keys with `dot < 0` make engines
//!   without neighborhood correction slerp the long way around. Negating
//!   a quaternion leaves the rotation it represents unchanged, so
//!   walking each track and negating keys until the whole track is
//!   hemisphere-consistent is lossless.
//!
//! Scope: float32 VEC4 rotation outputs. `quat-flip` handles CUBICSPLINE
//! tracks by negating the whole `[in-tangent, value, out-tangent]`
//! triplet with the key — the tangents are derivatives in quaternion
//! component space, so they flip with it. (Hermite segments between a
//! flipped and an unflipped key traverse 4-space differently than
//! authored, but the authored curve was the long-way spin being
//! repaired.) `quat-norm` skips CUBICSPLINE tracks because scaling value
//! keys without their tangents would change interior samples. Sparse
//! accessors and quantized (normalized-int) rotations are skipped.

use crate::{FixError, LoadError, WriteError, safe_external_buffer_path};
use base64::Engine as _;
use std::ops::Range;
use std::path::Path;

const ROTATION_ELEMENT_BYTES: usize = 16;
const QUAT_NORM_TOLERANCE: f32 = 1e-3;

/// One track's repair summary.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct TrackFix {
    pub clip: String,
    pub bone: String,
    pub fixed_keys: usize,
}

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct FixReport {
    pub tracks: Vec<TrackFix>,
    /// Tracks that needed repair but were skipped (data URI, cubic,
    /// quantized, sparse, or malformed accessors), with reasons.
    pub skipped: Vec<String>,
}

impl FixReport {
    pub fn total_fixed(&self) -> usize {
        self.tracks.iter().map(|t| t.fixed_keys).sum()
    }
}

/// Parsed input plus mutable buffer bytes for one `fix` run.
pub struct FixSession {
    original: Vec<u8>,
    gltf: gltf::Gltf,
    buffers: Vec<Vec<u8>>,
}

impl FixSession {
    /// Read and parse a glTF/GLB once, loading every declared buffer.
    pub fn read(input: &Path) -> Result<Self, FixError> {
        let original = std::fs::read(input).map_err(|source| LoadError::Io {
            path: input.display().to_string(),
            source,
        })?;
        crate::validate_glb_framing(&original)?;
        let gltf = gltf::Gltf::from_slice(&original).map_err(LoadError::from)?;
        crate::validate_animation_channels(gltf.document.as_json())?;

        // Buffers as mutable byte vectors, indexed as the JSON declares
        // them (the BIN-chunk buffer is located by Source::Bin at write
        // time, not assumed to be index 0). External and data-URI buffers
        // are loaded, patched, and written back to where they came from.
        let base = input.parent();
        let mut buffers: Vec<Vec<u8>> = Vec::new();
        for buffer in gltf.buffers() {
            let data = match buffer.source() {
                gltf::buffer::Source::Bin => gltf
                    .blob
                    .clone()
                    .ok_or_else(|| LoadError::Buffer("GLB has no BIN chunk".into()))?,
                gltf::buffer::Source::Uri(uri) => {
                    if let Some(encoded) = uri.strip_prefix("data:") {
                        let payload = encoded
                            .split_once("base64,")
                            .map(|(_, p)| p)
                            .ok_or_else(|| LoadError::Buffer("unsupported data URI".into()))?;
                        base64::engine::general_purpose::STANDARD
                            .decode(payload)
                            .map_err(|e| LoadError::Buffer(format!("bad base64 data URI: {e}")))?
                    } else {
                        let path = base
                            .unwrap_or(Path::new("."))
                            .join(safe_external_buffer_path(uri)?);
                        std::fs::read(&path).map_err(|source| LoadError::Io {
                            path: path.display().to_string(),
                            source,
                        })?
                    }
                }
            };
            buffers.push(data);
        }

        Ok(Self {
            original,
            gltf,
            buffers,
        })
    }

    /// Normalize every repairable finite, non-zero rotation key in memory.
    pub fn fix_quat_norm(&mut self) -> FixReport {
        self.repair_rotation_tracks(|buffer, layout| {
            if layout.cubic {
                let needs_repair = (0..layout.keys).any(|k| {
                    let value_element = k * layout.per_key + layout.value_offset;
                    let q = read_rotation(buffer, layout, value_element);
                    if !q.iter().all(|v| v.is_finite()) {
                        return false;
                    }
                    let len = q.iter().map(|v| v * v).sum::<f32>().sqrt();
                    len <= f32::EPSILON || (len - 1.0).abs() > QUAT_NORM_TOLERANCE
                });
                return (
                    0,
                    needs_repair.then_some(
                        "cubic rotation output (quat-norm skipped to preserve tangents)",
                    ),
                );
            }

            let mut fixed = 0usize;
            let mut skipped = None;
            for k in 0..layout.keys {
                let value_element = k * layout.per_key + layout.value_offset;
                let q = read_rotation(buffer, layout, value_element);
                if !q.iter().all(|v| v.is_finite()) {
                    skipped = Some("non-finite rotation key");
                    continue;
                }
                let len = q.iter().map(|v| v * v).sum::<f32>().sqrt();
                if len <= f32::EPSILON {
                    skipped = Some("zero-length rotation key");
                    continue;
                }
                if (len - 1.0).abs() > QUAT_NORM_TOLERANCE {
                    write_rotation(buffer, layout, value_element, q.map(|v| v / len));
                    fixed += 1;
                }
            }
            (fixed, skipped)
        })
    }

    /// Hemisphere-normalize every repairable rotation track in memory.
    pub fn fix_quat_hemisphere(&mut self) -> FixReport {
        self.repair_rotation_tracks(|buffer, layout| {
            let mut prev: Option<[f32; 4]> = None;
            let mut flipped = 0usize;
            for k in 0..layout.keys {
                let value_element = k * layout.per_key + layout.value_offset;
                let q = read_rotation(buffer, layout, value_element);
                if let Some(p) = prev {
                    let dot: f32 = p.iter().zip(&q).map(|(a, b)| a * b).sum();
                    if dot < 0.0 {
                        for e in (k * layout.per_key)..(k * layout.per_key + layout.per_key) {
                            let negated = read_rotation(buffer, layout, e).map(|v| -v);
                            write_rotation(buffer, layout, e, negated);
                        }
                        flipped += 1;
                        prev = Some(q.map(|v| -v));
                        continue;
                    }
                }
                prev = Some(q);
            }
            (flipped, None)
        })
    }

    fn repair_rotation_tracks(
        &mut self,
        mut repair: impl FnMut(&mut [u8], RotationLayout) -> (usize, Option<&'static str>),
    ) -> FixReport {
        let mut report = FixReport::default();
        let gltf = &self.gltf;
        let buffers = &mut self.buffers;
        for animation in gltf.animations() {
            let clip = animation.name().unwrap_or("<unnamed>").to_string();
            for channel in animation.channels() {
                if channel.target().property() != gltf::animation::Property::Rotation {
                    continue;
                }
                let bone = channel
                    .target()
                    .node()
                    .name()
                    .unwrap_or("<unnamed>")
                    .to_string();
                let sampler = channel.sampler();
                let cubic = sampler.interpolation() == gltf::animation::Interpolation::CubicSpline;
                let accessor = sampler.output();
                if accessor.sparse().is_some() {
                    report
                        .skipped
                        .push(format!("{clip}/{bone}: sparse accessor"));
                    continue;
                }
                if accessor.data_type() != gltf::accessor::DataType::F32
                    || accessor.dimensions() != gltf::accessor::Dimensions::Vec4
                {
                    report.skipped.push(format!(
                        "{clip}/{bone}: quantized rotation output ({:?})",
                        accessor.data_type()
                    ));
                    continue;
                }
                let Some(view) = accessor.view() else {
                    report
                        .skipped
                        .push(format!("{clip}/{bone}: accessor without view"));
                    continue;
                };
                let buffer_index = view.buffer().index();
                if matches!(
                    view.buffer().source(),
                    gltf::buffer::Source::Uri(uri) if uri.starts_with("data:")
                ) {
                    report.skipped.push(format!(
                        "{clip}/{bone}: data-URI buffer (convert to .glb first)"
                    ));
                    continue;
                }
                let stride = view.stride().unwrap_or(16);
                let Some(start) = view.offset().checked_add(accessor.offset()) else {
                    report
                        .skipped
                        .push(format!("{clip}/{bone}: accessor byte offset overflow"));
                    continue;
                };
                let Some(buffer) = buffers.get_mut(buffer_index) else {
                    report
                        .skipped
                        .push(format!("{clip}/{bone}: missing buffer {buffer_index}"));
                    continue;
                };

                // Cubic outputs hold [in-tangent, value, out-tangent]
                // triplets; repairs inspect VALUE elements, while
                // hemisphere flips negate whole triplets.
                let (per_key, value_offset) = if cubic { (3usize, 1usize) } else { (1, 0) };
                if accessor.count() % per_key != 0 {
                    report
                        .skipped
                        .push(format!("{clip}/{bone}: malformed cubic rotation accessor"));
                    continue;
                }
                let Some(range) =
                    accessor_byte_range(start, stride, accessor.count(), ROTATION_ELEMENT_BYTES)
                else {
                    report
                        .skipped
                        .push(format!("{clip}/{bone}: accessor byte range overflow"));
                    continue;
                };
                if range.end > buffer.len() {
                    report.skipped.push(format!(
                        "{clip}/{bone}: accessor byte range {}..{} outside buffer length {}",
                        range.start,
                        range.end,
                        buffer.len()
                    ));
                    continue;
                }
                let layout = RotationLayout {
                    start,
                    stride,
                    keys: accessor.count() / per_key,
                    per_key,
                    value_offset,
                    cubic,
                };
                let (fixed_keys, skipped) = repair(buffer, layout);
                if let Some(reason) = skipped {
                    report.skipped.push(format!("{clip}/{bone}: {reason}"));
                }
                if fixed_keys > 0 {
                    report.tracks.push(TrackFix {
                        clip: clip.clone(),
                        bone,
                        fixed_keys,
                    });
                }
            }
        }
        report
    }

    /// Write the patched buffers, preserving the original container bytes.
    pub fn write(&self, input: &Path, output: &Path) -> Result<(), FixError> {
        write_patched(input, output, &self.original, &self.gltf, &self.buffers)
    }
}

#[derive(Clone, Copy)]
struct RotationLayout {
    start: usize,
    stride: usize,
    keys: usize,
    per_key: usize,
    value_offset: usize,
    cubic: bool,
}

fn read_rotation(buffer: &[u8], layout: RotationLayout, element: usize) -> [f32; 4] {
    let at = layout.start + element * layout.stride;
    let mut q = [0f32; 4];
    for (c, slot) in q.iter_mut().enumerate() {
        let o = at + c * 4;
        *slot = f32::from_le_bytes(buffer[o..o + 4].try_into().expect("slice has four bytes"));
    }
    q
}

fn write_rotation(buffer: &mut [u8], layout: RotationLayout, element: usize, q: [f32; 4]) {
    let at = layout.start + element * layout.stride;
    for (c, v) in q.iter().enumerate() {
        let o = at + c * 4;
        buffer[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
}

/// Hemisphere-normalize every rotation track of `input`, writing the
/// (otherwise byte-identical) result to `output`. `input` and `output`
/// may be the same path.
pub fn fix_quat_hemisphere(input: &Path, output: &Path) -> Result<FixReport, FixError> {
    let mut session = FixSession::read(input)?;
    let report = session.fix_quat_hemisphere();
    session.write(input, output)?;
    Ok(report)
}

/// Unit-normalize every finite, non-zero LINEAR/STEP rotation key of
/// `input`, writing the (otherwise byte-identical) result to `output`.
/// `input` and `output` may be the same path.
pub fn fix_quat_norm(input: &Path, output: &Path) -> Result<FixReport, FixError> {
    let mut session = FixSession::read(input)?;
    let report = session.fix_quat_norm();
    session.write(input, output)?;
    Ok(report)
}

/// Inspect which rotation tracks would be hemisphere-normalized without
/// writing any bytes.
pub fn inspect_quat_hemisphere(input: &Path) -> Result<FixReport, FixError> {
    let mut session = FixSession::read(input)?;
    let report = session.fix_quat_hemisphere();
    Ok(report)
}

/// Inspect which rotation tracks would be unit-normalized without
/// writing any bytes.
pub fn inspect_quat_norm(input: &Path) -> Result<FixReport, FixError> {
    let mut session = FixSession::read(input)?;
    let report = session.fix_quat_norm();
    Ok(report)
}

/// Reassemble the container with the original structure and the
/// patched buffer bytes.
fn write_patched(
    input: &Path,
    output: &Path,
    original: &[u8],
    gltf: &gltf::Gltf,
    buffers: &[Vec<u8>],
) -> Result<(), FixError> {
    let io_err = |path: &Path| {
        let path = path.display().to_string();
        move |source: std::io::Error| {
            FixError::Write(WriteError::Io {
                path: path.clone(),
                source,
            })
        }
    };

    if original.starts_with(b"glTF") {
        // GLB: copy the header + JSON chunk verbatim, splice the
        // patched BIN chunk (same length — we only overwrote values).
        let json_len = read_u32_le(original, 12)?;
        let bin_chunk_start = 12usize
            .checked_add(8)
            .and_then(|n| n.checked_add(json_len))
            .ok_or_else(|| LoadError::Buffer("malformed GLB chunk length overflow".into()))?;
        if bin_chunk_start > original.len() {
            return Err(LoadError::Buffer("malformed GLB JSON chunk length".into()).into());
        }
        let mut out = original[..bin_chunk_start].to_vec();
        if bin_chunk_start < original.len() {
            let bin_len = read_u32_le(original, bin_chunk_start)?;
            let bin_header_end = bin_chunk_start
                .checked_add(8)
                .ok_or_else(|| LoadError::Buffer("malformed GLB BIN chunk overflow".into()))?;
            if bin_header_end > original.len() {
                return Err(LoadError::Buffer("malformed GLB BIN chunk header".into()).into());
            }
            out.extend_from_slice(&original[bin_chunk_start..bin_header_end]);
            // The BIN chunk holds the buffer with Source::Bin (buffer 0
            // per spec when present) — not blindly buffers[0], which
            // may be a URI buffer in a BIN-less GLB.
            let bin = gltf
                .buffers()
                .position(|b| matches!(b.source(), gltf::buffer::Source::Bin))
                .and_then(|i| buffers.get(i))
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            if bin.len() > bin_len {
                return Err(LoadError::Buffer(format!(
                    "patched BIN chunk length {} exceeds original length {bin_len}",
                    bin.len()
                ))
                .into());
            }
            out.extend_from_slice(bin);
            // Preserve the original chunk padding.
            let padding_start = bin_header_end
                .checked_add(bin.len())
                .ok_or_else(|| LoadError::Buffer("malformed GLB BIN chunk overflow".into()))?;
            if padding_start > original.len() {
                return Err(LoadError::Buffer("malformed GLB BIN chunk length".into()).into());
            }
            out.extend_from_slice(&original[padding_start..]);
        }
        std::fs::write(output, out).map_err(io_err(output))?;
        // A GLB may also reference external URI buffers; their patched
        // bytes must land on disk too, or "N keys fixed" is a false
        // success (the repaired keys would be in the untouched .bin).
        return write_uri_buffers(output, gltf, buffers);
    }

    // .gltf: the JSON is untouched; copy it through and write each
    // patched non-data-URI buffer back to its own file (resolved
    // against the OUTPUT location so `-o elsewhere/` keeps the pair
    // together).
    if input != output {
        std::fs::copy(input, output).map_err(io_err(output))?;
    }
    write_uri_buffers(output, gltf, buffers)
}

/// Write every patched external (non-data-URI) buffer next to
/// `output`, keeping the container/buffer pair together.
fn write_uri_buffers(
    output: &Path,
    gltf: &gltf::Gltf,
    buffers: &[Vec<u8>],
) -> Result<(), FixError> {
    for (buffer, data) in gltf.buffers().zip(buffers) {
        if let gltf::buffer::Source::Uri(uri) = buffer.source() {
            if uri.starts_with("data:") {
                continue; // never patched — such tracks are skipped upstream
            }
            let path = output
                .parent()
                .unwrap_or(Path::new("."))
                .join(safe_external_buffer_path(uri)?);
            std::fs::write(&path, data).map_err(|source| {
                FixError::Write(WriteError::Io {
                    path: path.display().to_string(),
                    source,
                })
            })?;
        }
    }
    Ok(())
}

fn accessor_byte_range(
    start: usize,
    stride: usize,
    element_count: usize,
    element_bytes: usize,
) -> Option<Range<usize>> {
    if stride < element_bytes {
        return None;
    }
    if element_count == 0 {
        return Some(start..start);
    }
    let last = element_count.checked_sub(1)?;
    let last_start = start.checked_add(last.checked_mul(stride)?)?;
    Some(start..last_start.checked_add(element_bytes)?)
}

fn read_u32_le(bytes: &[u8], offset: usize) -> Result<usize, LoadError> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| LoadError::Buffer("malformed GLB offset overflow".into()))?;
    let word = bytes
        .get(offset..end)
        .ok_or_else(|| LoadError::Buffer("malformed GLB chunk header".into()))?;
    Ok(u32::from_le_bytes(word.try_into().expect("slice has four bytes")) as usize)
}
