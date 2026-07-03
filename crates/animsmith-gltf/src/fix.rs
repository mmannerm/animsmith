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
//! Scope: LINEAR and STEP tracks with float32 VEC4 output. CUBICSPLINE
//! tracks are skipped with a warning (negating a key also changes how
//! its Hermite segments traverse 4-space; a correct cubic fix needs
//! resampling, which is not byte-surgical). Sparse accessors and
//! quantized (normalized-int) rotations are likewise skipped.

use crate::LoadError;
use base64::Engine as _;
use std::path::Path;

/// One track's repair summary.
#[derive(Debug, Clone)]
pub struct TrackFix {
    pub clip: String,
    pub bone: String,
    pub flipped_keys: usize,
}

#[derive(Debug, Clone, Default)]
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
pub fn fix_quat_hemisphere(input: &Path, output: &Path) -> Result<FixReport, LoadError> {
    let bytes = std::fs::read(input).map_err(|source| LoadError::Io {
        path: input.display().to_string(),
        source,
    })?;
    let gltf = gltf::Gltf::from_slice(&bytes)?;

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
                    let path = base.unwrap_or(Path::new(".")).join(uri);
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
            if sampler.interpolation() == gltf::animation::Interpolation::CubicSpline {
                report.skipped.push(format!(
                    "{clip}/{bone}: CUBICSPLINE track (needs resampling)"
                ));
                continue;
            }
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
            if is_data_uri[view.buffer().index()] {
                report.skipped.push(format!(
                    "{clip}/{bone}: data-URI buffer (convert to .glb first)"
                ));
                continue;
            }
            let stride = view.stride().unwrap_or(16);
            let start = view.offset() + accessor.offset();
            let buffer = &mut buffers[view.buffer().index()];

            let mut prev: Option<[f32; 4]> = None;
            let mut flipped = 0usize;
            for k in 0..accessor.count() {
                let at = start + k * stride;
                let mut q = [0f32; 4];
                for (c, slot) in q.iter_mut().enumerate() {
                    let o = at + c * 4;
                    *slot = f32::from_le_bytes(buffer[o..o + 4].try_into().unwrap());
                }
                if let Some(p) = prev {
                    let dot: f32 = p.iter().zip(&q).map(|(a, b)| a * b).sum();
                    if dot < 0.0 {
                        for slot in q.iter_mut() {
                            *slot = -*slot;
                        }
                        for (c, slot) in q.iter().enumerate() {
                            let o = at + c * 4;
                            buffer[o..o + 4].copy_from_slice(&slot.to_le_bytes());
                        }
                        flipped += 1;
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

    write_patched(input, output, &bytes, &gltf, buffers)?;
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
) -> Result<(), LoadError> {
    let io_err = |path: &Path| {
        let path = path.display().to_string();
        move |source: std::io::Error| LoadError::Io {
            path: path.clone(),
            source,
        }
    };

    if original.starts_with(b"glTF") {
        // GLB: copy the header + JSON chunk verbatim, splice the
        // patched BIN chunk (same length — we only overwrote values).
        let json_len = u32::from_le_bytes(original[12..16].try_into().unwrap()) as usize;
        let bin_chunk_start = 12 + 8 + json_len;
        let mut out = original[..bin_chunk_start].to_vec();
        if bin_chunk_start < original.len() {
            let bin_len = u32::from_le_bytes(
                original[bin_chunk_start..bin_chunk_start + 4]
                    .try_into()
                    .unwrap(),
            ) as usize;
            out.extend_from_slice(&original[bin_chunk_start..bin_chunk_start + 8]);
            let bin = buffers.first().cloned().unwrap_or_default();
            debug_assert!(bin.len() <= bin_len);
            out.extend_from_slice(&bin);
            // Preserve the original chunk padding.
            out.extend_from_slice(&original[bin_chunk_start + 8 + bin.len()..]);
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
            let path = output.parent().unwrap_or(Path::new(".")).join(uri);
            std::fs::write(&path, data).map_err(io_err(&path))?;
        }
    }
    Ok(())
}
