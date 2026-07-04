//! Byte-surgical clip repairs: mutate only the animation accessor
//! bytes that need to change and copy everything else through
//! verbatim. A fixed character GLB keeps its meshes, skins, materials,
//! and textures byte-identical — the output diff is exactly the
//! repaired keys.
//!
//! First (and so far only) fix: quaternion hemisphere normalization.
//! Adjacent rotation keys with `dot < 0` make engines without
//! neighborhood correction slerp the long way around (the `quat-flip`
//! check). Negating a quaternion leaves the rotation it represents
//! unchanged, so walking each track and negating keys until the whole
//! track is hemisphere-consistent is lossless.
//!
//! Scope: float32 VEC4 rotation outputs. For CUBICSPLINE tracks the
//! whole `[in-tangent, value, out-tangent]` triplet is negated with the
//! key — the tangents are derivatives in quaternion component space, so
//! they flip with it. (Hermite segments between a flipped and an
//! unflipped key traverse 4-space differently than authored, but the
//! authored curve was the long-way spin being repaired.) Sparse
//! accessors and quantized (normalized-int) rotations are skipped.

use crate::{FixError, LoadError, WriteError};
use base64::Engine as _;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

const ROTATION_ELEMENT_BYTES: usize = 16;

/// One track's repair summary.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct TrackFix {
    pub clip: String,
    pub bone: String,
    pub flipped_keys: usize,
}

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct FixReport {
    pub tracks: Vec<TrackFix>,
    /// Tracks that needed repair but were skipped (cubic, quantized,
    /// sparse), with reasons.
    pub skipped: Vec<String>,
}

impl FixReport {
    pub fn total_flipped(&self) -> usize {
        self.tracks.iter().map(|t| t.flipped_keys).sum()
    }
}

/// Hemisphere-normalize every rotation track of `input`, writing the
/// (otherwise byte-identical) result to `output`. `input` and `output`
/// may be the same path.
pub fn fix_quat_hemisphere(input: &Path, output: &Path) -> Result<FixReport, FixError> {
    fix_quat_hemisphere_impl(input, Some(output))
}

/// Inspect which rotation tracks would be hemisphere-normalized without
/// writing any bytes.
pub fn inspect_quat_hemisphere(input: &Path) -> Result<FixReport, FixError> {
    fix_quat_hemisphere_impl(input, None)
}

fn fix_quat_hemisphere_impl(input: &Path, output: Option<&Path>) -> Result<FixReport, FixError> {
    let bytes = std::fs::read(input).map_err(|source| LoadError::Io {
        path: input.display().to_string(),
        source,
    })?;
    let gltf = gltf::Gltf::from_slice(&bytes).map_err(LoadError::from)?;

    // Buffers as mutable byte vectors; index 0 of a GLB is the BIN
    // chunk. External and data-URI buffers are loaded, patched, and
    // written back to where they came from.
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

    let is_data_uri: Vec<bool> = gltf
        .buffers()
        .map(|b| matches!(b.source(), gltf::buffer::Source::Uri(u) if u.starts_with("data:")))
        .collect();

    let mut report = FixReport::default();
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
            let Some(is_data_uri_buffer) = is_data_uri.get(buffer_index).copied() else {
                report
                    .skipped
                    .push(format!("{clip}/{bone}: buffer index out of range"));
                continue;
            };
            if is_data_uri_buffer {
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
            // triplets; the hemisphere walk compares VALUE elements and
            // negates whole triplets.
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
            let keys = accessor.count() / per_key;
            let read4 = |buffer: &[u8], element: usize| -> [f32; 4] {
                let at = start + element * stride;
                let mut q = [0f32; 4];
                for (c, slot) in q.iter_mut().enumerate() {
                    let o = at + c * 4;
                    *slot = f32::from_le_bytes(buffer[o..o + 4].try_into().unwrap());
                }
                q
            };
            let mut prev: Option<[f32; 4]> = None;
            let mut flipped = 0usize;
            for k in 0..keys {
                let value_element = k * per_key + value_offset;
                let q = read4(buffer, value_element);
                if let Some(p) = prev {
                    let dot: f32 = p.iter().zip(&q).map(|(a, b)| a * b).sum();
                    if dot < 0.0 {
                        for e in (k * per_key)..(k * per_key + per_key) {
                            let negated = read4(buffer, e).map(|v| -v);
                            let at = start + e * stride;
                            for (c, v) in negated.iter().enumerate() {
                                let o = at + c * 4;
                                buffer[o..o + 4].copy_from_slice(&v.to_le_bytes());
                            }
                        }
                        flipped += 1;
                        prev = Some(q.map(|v| -v));
                        continue;
                    }
                }
                prev = Some(q);
            }
            if flipped > 0 {
                report.tracks.push(TrackFix {
                    clip: clip.clone(),
                    bone,
                    flipped_keys: flipped,
                });
            }
        }
    }

    if let Some(output) = output {
        write_patched(input, output, &bytes, &gltf, buffers)?;
    }
    Ok(report)
}

/// Reassemble the container with the original structure and the
/// patched buffer bytes.
fn write_patched(
    input: &Path,
    output: &Path,
    original: &[u8],
    gltf: &gltf::Gltf,
    buffers: Vec<Vec<u8>>,
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
            .ok_or_else(|| {
                FixError::Load(LoadError::Buffer(
                    "malformed GLB chunk length overflow".into(),
                ))
            })?;
        if bin_chunk_start > original.len() {
            return Err(LoadError::Buffer("malformed GLB JSON chunk length".into()).into());
        }
        let mut out = original[..bin_chunk_start].to_vec();
        if bin_chunk_start < original.len() {
            let bin_len = read_u32_le(original, bin_chunk_start)?;
            let bin_header_end = bin_chunk_start.checked_add(8).ok_or_else(|| {
                FixError::Load(LoadError::Buffer("malformed GLB BIN chunk overflow".into()))
            })?;
            if bin_header_end > original.len() {
                return Err(LoadError::Buffer("malformed GLB BIN chunk header".into()).into());
            }
            out.extend_from_slice(&original[bin_chunk_start..bin_header_end]);
            let bin = buffers.first().cloned().unwrap_or_default();
            if bin.len() > bin_len {
                return Err(LoadError::Buffer(format!(
                    "patched BIN chunk length {} exceeds original length {bin_len}",
                    bin.len()
                ))
                .into());
            }
            out.extend_from_slice(&bin);
            // Preserve the original chunk padding.
            let padding_start = bin_header_end.checked_add(bin.len()).ok_or_else(|| {
                FixError::Load(LoadError::Buffer("malformed GLB BIN chunk overflow".into()))
            })?;
            if padding_start > original.len() {
                return Err(LoadError::Buffer("malformed GLB BIN chunk length".into()).into());
            }
            out.extend_from_slice(&original[padding_start..]);
        }
        return std::fs::write(output, out).map_err(io_err(output));
    }

    // .gltf: the JSON is untouched; copy it through and write each
    // patched non-data-URI buffer back to its own file (resolved
    // against the OUTPUT location so `-o elsewhere/` keeps the pair
    // together).
    if input != output {
        std::fs::copy(input, output).map_err(io_err(output))?;
    }
    for (buffer, data) in gltf.buffers().zip(buffers) {
        if let gltf::buffer::Source::Uri(uri) = buffer.source() {
            if uri.starts_with("data:") {
                continue; // never patched — such tracks are skipped upstream
            }
            let path = output
                .parent()
                .unwrap_or(Path::new("."))
                .join(safe_external_buffer_path(uri)?);
            std::fs::write(&path, data).map_err(io_err(&path))?;
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

fn safe_external_buffer_path(uri: &str) -> Result<PathBuf, LoadError> {
    if uri.is_empty() || uri.contains('\\') {
        return Err(LoadError::Buffer(format!(
            "unsafe external buffer URI {uri:?}: expected a relative child path"
        )));
    }
    let path = Path::new(uri);
    if path.is_absolute() {
        return Err(LoadError::Buffer(format!(
            "unsafe external buffer URI {uri:?}: absolute paths are not supported"
        )));
    }
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => out.push(part),
            _ => {
                return Err(LoadError::Buffer(format!(
                    "unsafe external buffer URI {uri:?}: expected a relative child path"
                )));
            }
        }
    }
    if out.as_os_str().is_empty() {
        return Err(LoadError::Buffer(format!(
            "unsafe external buffer URI {uri:?}: expected a relative child path"
        )));
    }
    Ok(out)
}
