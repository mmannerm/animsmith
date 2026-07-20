//! [`load`] reads FBX files into an
//! [`animsmith_core::Document`], normalizing parser errors into
//! [`LoadError`]. The resulting document carries skeletons, animation
//! clips, and scene assets in the same core model used by the glTF
//! loader.
//!
//! The loader normalizes FBX scenes into animsmith's runtime-oriented
//! coordinate space before handing them to `animsmith-core`: right-handed
//! +Y-up axes, metres, transform-adjust conversion, helper nodes for
//! geometric transforms, and compensated scale inheritance. Depend on this
//! crate only when your pipeline accepts FBX input; it brings the bundled
//! `ufbx` C build that `animsmith-core` and `animsmith-gltf` intentionally
//! avoid.
//!
//! # Quick start
//!
//! ```no_run
//! fn lint_fbx(
//!     path: &std::path::Path,
//! ) -> Result<Vec<animsmith_core::Finding>, Box<dyn std::error::Error>> {
//!     let doc = animsmith_fbx::load(path)?;
//!     let roles = animsmith_core::detect_profile(&doc.skeleton).unwrap_or_default();
//!     let config = animsmith_core::Config::default();
//!     let grids = animsmith_core::MetricGrids::new(&doc);
//!     let ctx = animsmith_core::CheckCtx::new(&grids, &roles, &config);
//!     let results = animsmith_core::evaluate_checks(
//!         &ctx,
//!         &animsmith_core::all_checks(),
//!         animsmith_core::CheckSelection::All,
//!     )?;
//!     Ok(results.into_iter().flat_map(|check| check.findings).collect())
//! }
//! ```
//!
//! # Build and API status
//!
//! The library crate has no public feature flags and supports the workspace
//! MSRV, Rust 1.88. It includes the bundled `ufbx` C build. Its Rust API is
//! pre-1.0; see `animsmith-core`'s crate-level API status for the shared
//! stability boundary.
//!
//! See the GitHub [embedding guide] for crate selection and the [pipeline
//! scenario guide] for FBX intake and conversion workflows.
//!
//! [embedding guide]: https://github.com/mmannerm/animsmith/blob/main/docs/embedding.md
//! [pipeline scenario guide]: https://github.com/mmannerm/animsmith/blob/main/docs/pipeline-scenarios.md
//!
#![warn(missing_docs)]

use animsmith_core::model::{
    Bone, Clip, Document, Interpolation, MaterialAsset, MeshAsset, Primitive, Property,
    SceneAssets, Skeleton, SourceInfo, TextureAsset, Track, TrackValues, Transform,
};
use glam::{Mat4, Quat, Vec3};
use std::path::Path;

/// Errors returned while loading an FBX scene into the core model.
///
/// These errors describe input or parser failures. They do not represent
/// animation check findings; once a [`Document`] loads, semantic problems
/// are reported by `animsmith-core` checks instead.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoadError {
    /// The input path could not be represented as UTF-8 for `ufbx`.
    #[error("path is not valid UTF-8: {0}")]
    Path(String),
    /// `ufbx` rejected or could not parse the file.
    #[error("FBX parse error: {0}")]
    Fbx(String),
    /// `ufbx` loaded the scene but failed while baking an animation take.
    #[error("animation bake failed for take {take:?}: {message}")]
    Bake {
        /// Name of the animation take that failed to bake.
        take: String,
        /// Parser-provided bake failure detail.
        message: String,
    },
}

fn vec3(v: ufbx::Vec3) -> Vec3 {
    Vec3::new(v.x as f32, v.y as f32, v.z as f32)
}

fn quat(q: ufbx::Quat) -> Quat {
    Quat::from_xyzw(q.x as f32, q.y as f32, q.z as f32, q.w as f32)
}

fn transform(t: &ufbx::Transform) -> Transform {
    Transform {
        translation: vec3(t.translation),
        rotation: quat(t.rotation),
        scale: vec3(t.scale),
    }
}

/// ufbx matrices are 3×4 (rotation/scale columns + translation).
fn mat4(m: &ufbx::Matrix) -> Mat4 {
    Mat4::from_cols_array(&[
        m.m00 as f32,
        m.m10 as f32,
        m.m20 as f32,
        0.0,
        m.m01 as f32,
        m.m11 as f32,
        m.m21 as f32,
        0.0,
        m.m02 as f32,
        m.m12 as f32,
        m.m22 as f32,
        0.0,
        m.m03 as f32,
        m.m13 as f32,
        m.m23 as f32,
        1.0,
    ])
}

/// Load an `.fbx` file into a core [`Document`]: skeleton, animation,
/// and scene assets (triangulated meshes, skins, factor-only
/// materials). Consumers that only judge animation ignore
/// [`Document::assets`].
///
/// # Errors
///
/// Returns [`LoadError::Path`] when the path cannot be passed to `ufbx`,
/// [`LoadError::Fbx`] when the FBX container cannot be parsed, and
/// [`LoadError::Bake`] when an animation stack cannot be baked into the
/// linear TRS tracks that animsmith's checks consume.
pub fn load(path: &Path) -> Result<Document, LoadError> {
    let filename = path
        .to_str()
        .ok_or_else(|| LoadError::Path(path.display().to_string()))?;
    let opts = ufbx::LoadOpts {
        target_axes: ufbx::CoordinateAxes::right_handed_y_up(),
        target_unit_meters: 1.0,
        space_conversion: ufbx::SpaceConversion::AdjustTransforms,
        geometry_transform_handling: ufbx::GeometryTransformHandling::HelperNodes,
        // FBX scale-compensation inheritance (Maya-style; ubiquitous in
        // Mixamo rigs, every bone carrying scale 0.01) cannot be
        // represented by plain TRS hierarchies like glTF's — ufbx
        // compensates the transforms (with helper nodes as fallback)
        // so standard composition is correct.
        inherit_mode_handling: ufbx::InheritModeHandling::Compensate,
        generate_missing_normals: true,
        ..Default::default()
    };
    let scene = ufbx::load_file(filename, opts).map_err(|e| LoadError::Fbx(format!("{e:?}")))?;

    // Every node becomes a bone (the ufbx root included — it carries
    // the axis/unit adjustment). scene.nodes is ordered parents-first,
    // matching the skeleton invariant; typed_id indexes scene.nodes
    // directly.
    let mut bones: Vec<Bone> = Vec::with_capacity(scene.nodes.len());
    for node in &scene.nodes {
        let name = if node.element.name.is_empty() {
            if node.is_root {
                "<fbx-root>".to_string()
            } else {
                format!("node{}", node.element.typed_id)
            }
        } else {
            node.element.name.to_string()
        };
        bones.push(Bone {
            name,
            parent: node.parent.as_ref().map(|p| p.element.typed_id as usize),
            rest: transform(&node.local_transform),
            inverse_bind: None,
        });
    }
    for cluster in &scene.skin_clusters {
        if let Some(bone_node) = &cluster.bone_node {
            let id = bone_node.element.typed_id as usize;
            if id < bones.len() {
                // Joint-centric bind inverse in the converted scene
                // space; the mesh-dependent part lives per mesh in
                // `MeshAsset::skin_ibms`.
                bones[id].inverse_bind = Some(mat4(&cluster.bind_to_world).inverse());
            }
        }
    }

    let mut clips = Vec::new();
    for (index, stack) in scene.anim_stacks.iter().enumerate() {
        let take = if stack.element.name.is_empty() {
            format!("take{index}")
        } else {
            stack.element.name.to_string()
        };
        let baked = ufbx::bake_anim(
            &scene,
            &stack.anim,
            ufbx::BakeOpts {
                trim_start_time: true,
                ..Default::default()
            },
        )
        .map_err(|e| LoadError::Bake {
            take: take.clone(),
            message: format!("{e:?}"),
        })?;

        let mut tracks = Vec::new();
        let mut duration = 0.0f64;
        for node in &baked.nodes {
            let bone = node.typed_id as usize;
            if !node.translation_keys.is_empty() {
                let times: Vec<f32> = node
                    .translation_keys
                    .iter()
                    .map(|k| k.time as f32)
                    .collect();
                let values: Vec<Vec3> = node
                    .translation_keys
                    .iter()
                    .map(|k| vec3(k.value))
                    .collect();
                duration = duration.max(times.last().copied().unwrap_or(0.0) as f64);
                tracks.push(Track {
                    bone,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times,
                    values: TrackValues::Vec3s(values),
                });
            }
            if !node.rotation_keys.is_empty() {
                let times: Vec<f32> = node.rotation_keys.iter().map(|k| k.time as f32).collect();
                let values: Vec<Quat> = node.rotation_keys.iter().map(|k| quat(k.value)).collect();
                duration = duration.max(times.last().copied().unwrap_or(0.0) as f64);
                tracks.push(Track {
                    bone,
                    property: Property::Rotation,
                    interpolation: Interpolation::Linear,
                    times,
                    values: TrackValues::Quats(values),
                });
            }
            if !node.scale_keys.is_empty() {
                let times: Vec<f32> = node.scale_keys.iter().map(|k| k.time as f32).collect();
                let values: Vec<Vec3> = node.scale_keys.iter().map(|k| vec3(k.value)).collect();
                duration = duration.max(times.last().copied().unwrap_or(0.0) as f64);
                tracks.push(Track {
                    bone,
                    property: Property::Scale,
                    interpolation: Interpolation::Linear,
                    times,
                    values: TrackValues::Vec3s(values),
                });
            }
        }
        clips.push(Clip {
            name: take,
            duration_s: duration,
            tracks,
        });
    }

    let assets = extract_assets(&scene, path.parent());

    Ok(Document {
        skeleton: Skeleton { bones },
        clips,
        assets,
        source: SourceInfo {
            path: Some(path.display().to_string()),
            format: Some("fbx".into()),
        },
    })
}

/// Triangulated, unindexed geometry + skins + factor-only materials.
/// Corner attributes come straight from ufbx's indexed accessors; skin
/// weights are per source vertex (top four, renormalized).
/// Encoded image bytes for a material's base-color texture: embedded
/// FBX content first, else the referenced file next to the source.
/// Only PNG/JPEG pass through (glTF's mandated formats).
fn base_color_texture(material: &ufbx::Material, base_dir: Option<&Path>) -> Option<TextureAsset> {
    let texture = material.pbr.base_color.texture.as_ref().or(material
        .fbx
        .diffuse_color
        .texture
        .as_ref())?;
    let bytes: Vec<u8> = if !texture.content.is_empty() {
        texture.content.to_vec()
    } else {
        let mut found = None;
        for candidate in [
            texture.absolute_filename.as_ref(),
            texture.relative_filename.as_ref(),
            texture.filename.as_ref(),
        ] {
            if candidate.is_empty() {
                continue;
            }
            let direct = Path::new(candidate);
            let path = if direct.is_absolute() {
                direct.to_path_buf()
            } else {
                base_dir.unwrap_or(Path::new(".")).join(direct)
            };
            if let Ok(data) = std::fs::read(&path) {
                found = Some(data);
                break;
            }
        }
        found?
    };
    let mime = match bytes.get(..3) {
        Some([0x89, b'P', b'N']) => "image/png",
        Some([0xFF, 0xD8, _]) => "image/jpeg",
        _ => return None,
    };
    Some(TextureAsset {
        bytes,
        mime: mime.into(),
    })
}

fn extract_assets(scene: &ufbx::Scene, base_dir: Option<&Path>) -> SceneAssets {
    let mut assets = SceneAssets::default();
    let mut material_index: std::collections::BTreeMap<u32, usize> =
        std::collections::BTreeMap::new();

    for node in &scene.nodes {
        let Some(mesh) = &node.mesh else { continue };
        let node_id = node.element.typed_id as usize;

        // Materials referenced by this mesh, deduped globally by id.
        let local_materials: Vec<usize> = mesh
            .materials
            .iter()
            .map(|m| {
                *material_index
                    .entry(m.element.element_id)
                    .or_insert_with(|| {
                        let base = if m.pbr.base_color.has_value {
                            m.pbr.base_color.value_vec4
                        } else {
                            m.fbx.diffuse_color.value_vec4
                        };
                        let texture = base_color_texture(m, base_dir);
                        assets.materials.push(MaterialAsset {
                            name: m.element.name.to_string(),
                            // Exporter convention: a texture replaces
                            // the factor (they multiply in glTF).
                            base_color: if texture.is_some() {
                                [1.0, 1.0, 1.0, 1.0]
                            } else {
                                [base.x as f32, base.y as f32, base.z as f32, base.w as f32]
                            },
                            metallic: if m.pbr.metalness.has_value {
                                m.pbr.metalness.value_vec4.x as f32
                            } else {
                                0.0
                            },
                            roughness: if m.pbr.roughness.has_value {
                                m.pbr.roughness.value_vec4.x as f32
                            } else {
                                1.0
                            },
                            base_color_texture: texture,
                        });
                        assets.materials.len() - 1
                    })
            })
            .collect();

        // Per-vertex skin influences (top 4, renormalized), cluster
        // order defines the joint list.
        let skin = mesh.skin_deformers.first();
        let skin_joints: Vec<usize> = skin
            .map(|s| {
                s.clusters
                    .iter()
                    .map(|c| {
                        c.bone_node
                            .as_ref()
                            .map(|b| b.element.typed_id as usize)
                            .unwrap_or(0)
                    })
                    .collect()
            })
            .unwrap_or_default();
        // glTF inverse bind per joint: bind-world⁻¹ × geometry-to-world,
        // both already in ufbx's converted (metres, Y-up) space —
        // `geometry_to_bone` is raw source units and NOT suitable.
        let skin_ibms: Vec<glam::Mat4> = skin
            .map(|s| {
                s.clusters
                    .iter()
                    .map(|c| mat4(&c.bind_to_world).inverse() * mat4(&c.geometry_to_world))
                    .collect()
            })
            .unwrap_or_default();
        let vertex_influences: Vec<([u16; 4], [f32; 4])> = skin
            .map(|s| {
                (0..mesh.num_vertices)
                    .map(|v| {
                        let mut pairs: Vec<(u16, f32)> = Vec::new();
                        if let Some(sv) = s.vertices.get(v) {
                            for w in 0..sv.num_weights as usize {
                                let sw = &s.weights[sv.weight_begin as usize + w];
                                pairs.push((sw.cluster_index as u16, sw.weight as f32));
                            }
                        }
                        pairs.sort_by(|a, b| b.1.total_cmp(&a.1));
                        pairs.truncate(4);
                        let total: f32 = pairs.iter().map(|p| p.1).sum();
                        let mut joints = [0u16; 4];
                        let mut weights = [0f32; 4];
                        for (slot, (j, w)) in pairs.into_iter().enumerate() {
                            joints[slot] = j;
                            weights[slot] = if total > 0.0 { w / total } else { 0.0 };
                        }
                        (joints, weights)
                    })
                    .collect()
            })
            .unwrap_or_default();

        // One primitive per material slot (unindexed corners).
        let slots = local_materials.len().max(1);
        let mut primitives: Vec<Primitive> = (0..slots)
            .map(|slot| Primitive {
                material: local_materials.get(slot).copied(),
                ..Primitive::default()
            })
            .collect();

        let mut tri_indices = vec![0u32; mesh.max_face_triangles * 3];
        for (face_index, &face) in mesh.faces.iter().enumerate() {
            let slot = mesh
                .face_material
                .get(face_index)
                .map(|&m| m as usize)
                .filter(|&m| m < slots)
                .unwrap_or(0);
            let prim = &mut primitives[slot];
            let tris = mesh.triangulate_face(&mut tri_indices, face) as usize;
            for &corner in &tri_indices[..tris * 3] {
                let corner = corner as usize;
                let p = mesh.vertex_position[corner];
                prim.positions
                    .push(Vec3::new(p.x as f32, p.y as f32, p.z as f32));
                if mesh.vertex_normal.exists {
                    let n = mesh.vertex_normal[corner];
                    prim.normals
                        .push(Vec3::new(n.x as f32, n.y as f32, n.z as f32));
                }
                if mesh.vertex_uv.exists {
                    let uv = mesh.vertex_uv[corner];
                    // glTF's texcoord origin is top-left; FBX's is
                    // bottom-left.
                    prim.uvs.push([uv.x as f32, 1.0 - uv.y as f32]);
                }
                if !vertex_influences.is_empty() {
                    let vertex = mesh.vertex_indices[corner] as usize;
                    let (joints, weights) = vertex_influences[vertex];
                    prim.joints.push(joints);
                    prim.weights.push(weights);
                }
            }
        }
        primitives.retain(|p| !p.positions.is_empty());
        for prim in &mut primitives {
            prim.weld();
        }
        if primitives.is_empty() {
            continue;
        }
        assets.meshes.push(MeshAsset {
            name: mesh.element.name.to_string(),
            node: node_id,
            primitives,
            skin_joints,
            skin_ibms,
        });
    }
    assets
}
