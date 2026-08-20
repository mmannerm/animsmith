//! [`load`] and [`load_bytes`] read FBX input into an
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
//! [`load_scale_source`] and [`load_scale_source_bytes`] retain a typed
//! [`FbxScaleCapabilityInventory`] from the same parse. It gives every current
//! Appendix D.4 domain an explicit status and records baked curves, normalized
//! transforms, derived binds, truncated/renormalized influences,
//! triangulation, welding, generated data, and unavailable raw-span proof.
//! [`capability_facts`] remains the conservative generic refusal projection.
//! [`rest_bind_capability_facts`] admits only the complete normalized subset
//! used by the CLI's narrow FBX rest/bind path: it stages a new GLB, proves
//! that emitted GLB, and never claims raw FBX preservation. Whole-document
//! FBX scaling remains refused.
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
//!     Ok(results
//!         .into_iter()
//!         .flat_map(|check| check.findings().to_vec())
//!         .collect())
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

mod capability;

pub use capability::{
    FbxBindMatrixProvenance, FbxCoordinateAxis, FbxCoordinateNormalization,
    FbxScaleCapabilityInventory, FbxScaleDomainInventory, FbxScaleDomainStatus, FbxScaleSource,
    FbxSourceIdentity, capability_facts, rest_bind_capability_facts,
};

use animsmith_core::model::{
    Bone, Clip, Document, Interpolation, MaterialAsset, MeshAsset, MeshInstance,
    NormalTextureAsset, Primitive, Property, SceneAsset, SceneAssets, Skeleton, SourceInfo,
    SourceInverseBindAccessor, SourceInverseBindAccessorStatus, SourceNodeAsset,
    SourceNodeLocalRest, SourceSkeletonAssets, SourceSkeletonCoverage, SourceSkinAsset,
    SourceSkinAttachment, TextureAsset, Track, TrackValues, Transform,
};
use capability::AssetConversionFacts;
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

/// Project one converted FBX cluster bind only when the complete derivation
/// is finite. `Mat4::inverse()` returns non-finite components for a singular
/// finite input, so checking the two inputs alone is not sufficient evidence.
fn project_cluster_bind(cluster: &ufbx::SkinCluster) -> Option<(Mat4, Mat4)> {
    cluster.bone_node.as_ref()?;
    let bind_to_world = mat4(&cluster.bind_to_world);
    let geometry_to_world = mat4(&cluster.geometry_to_world);
    if !bind_to_world.is_finite() || !geometry_to_world.is_finite() {
        return None;
    }
    let bone_inverse = bind_to_world.inverse();
    let instance_inverse = bone_inverse * geometry_to_world;
    (bone_inverse.is_finite() && instance_inverse.is_finite())
        .then_some((bone_inverse, instance_inverse))
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
    Ok(load_scale_source(path)?.into_document())
}

/// Load an `.fbx` file and retain its conservative scale capability inventory.
///
/// The returned source is inventory-only: neither scale operation is enabled
/// for FBX by this API.
///
/// # Errors
///
/// Returns [`LoadError::Path`] when the path cannot be passed to `ufbx`,
/// [`LoadError::Fbx`] when the FBX container cannot be parsed, and
/// [`LoadError::Bake`] when an animation take cannot be baked.
pub fn load_scale_source(path: &Path) -> Result<FbxScaleSource, LoadError> {
    path.to_str()
        .ok_or_else(|| LoadError::Path(path.display().to_string()))?;
    let bytes = std::fs::read(path).map_err(|error| LoadError::Fbx(error.to_string()))?;
    load_scale_source_bytes(path, &bytes)
}

/// Load an FBX byte slice into a core [`Document`].
///
/// `bytes` supplies the top-level container exactly as captured by the
/// caller. `path` is retained for source provenance, diagnostics, and
/// resolving external resources relative to its parent directory.
///
/// # Errors
///
/// Returns [`LoadError::Path`] when `path` cannot be passed to `ufbx`,
/// [`LoadError::Fbx`] when the FBX container cannot be parsed, and
/// [`LoadError::Bake`] when an animation stack cannot be baked into the
/// linear TRS tracks that animsmith's checks consume.
pub fn load_bytes(path: &Path, bytes: &[u8]) -> Result<Document, LoadError> {
    Ok(load_scale_source_bytes(path, bytes)?.into_document())
}

/// Load captured FBX bytes and retain the capability inventory from the same parse.
///
/// `path` supplies source provenance and the base for external resources;
/// `bytes` is the exact captured top-level FBX container.
///
/// # Errors
///
/// Returns [`LoadError::Path`] when `path` cannot be passed to `ufbx`,
/// [`LoadError::Fbx`] when the FBX container cannot be parsed, and
/// [`LoadError::Bake`] when an animation take cannot be baked.
pub fn load_scale_source_bytes(path: &Path, bytes: &[u8]) -> Result<FbxScaleSource, LoadError> {
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
        filename: filename.into(),
        ..Default::default()
    };
    let scene = ufbx::load_memory(bytes, opts).map_err(|e| LoadError::Fbx(format!("{e:?}")))?;

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
        if let (Some(bone_node), Some((bone_inverse, _))) =
            (&cluster.bone_node, project_cluster_bind(cluster))
        {
            let id = bone_node.element.typed_id as usize;
            if id < bones.len() {
                // Joint-centric bind inverse in the converted scene
                // space; the mesh-dependent part lives per mesh in
                // `MeshAsset::skin_ibms`.
                bones[id].inverse_bind = Some(bone_inverse);
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

    let (assets, conversion) = extract_assets(&scene, path.parent());
    let inventory = capability::inventory(&scene, &conversion);

    Ok(FbxScaleSource {
        document: Document {
            skeleton: Skeleton { bones },
            clips,
            assets,
            source: SourceInfo {
                path: Some(path.display().to_string()),
                format: Some("fbx".into()),
            },
        },
        inventory,
    })
}

/// Read one ufbx texture: embedded FBX content first, else a referenced file
/// next to the source. Only PNG/JPEG pass through (glTF's mandated formats).
fn texture_asset(texture: &ufbx::Texture, base_dir: Option<&Path>) -> Option<TextureAsset> {
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

fn base_color_texture(material: &ufbx::Material, base_dir: Option<&Path>) -> Option<TextureAsset> {
    let texture = material.pbr.base_color.texture.as_ref().or(material
        .fbx
        .diffuse_color
        .texture
        .as_ref())?;
    texture_asset(texture, base_dir)
}

fn normal_texture(
    material: &ufbx::Material,
    base_dir: Option<&Path>,
) -> Option<NormalTextureAsset> {
    let texture = material.pbr.normal_map.texture.as_ref().or(material
        .fbx
        .normal_map
        .texture
        .as_ref())?;
    texture_asset(texture, base_dir).map(|texture| NormalTextureAsset {
        texture,
        // ufbx exposes the linked image but no glTF-compatible normal X/Y
        // scalar for ordinary FBX materials. Preserve the image and use the
        // glTF default rather than guessing from unrelated bump fields.
        scale: 1.0,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ProjectedInfluence {
    Absent,
    Retained(u16, f32),
    Rejected,
}

fn project_skin_influence(
    source_weight: f64,
    cluster_index: Option<usize>,
    cluster_count: usize,
    cluster_has_bone: bool,
) -> ProjectedInfluence {
    let weight = source_weight as f32;
    if !source_weight.is_finite()
        || source_weight < 0.0
        || !weight.is_finite()
        || (source_weight > 0.0 && weight == 0.0)
    {
        return ProjectedInfluence::Rejected;
    }
    if weight == 0.0 {
        return ProjectedInfluence::Absent;
    }
    let Some(cluster_index) = cluster_index else {
        return ProjectedInfluence::Rejected;
    };
    if cluster_index >= cluster_count || !cluster_has_bone {
        return ProjectedInfluence::Rejected;
    }
    match u16::try_from(cluster_index) {
        Ok(index) => ProjectedInfluence::Retained(index, weight),
        Err(_) => ProjectedInfluence::Rejected,
    }
}

/// Project every normalized ufbx node and skin deformer in stable typed-list
/// order. These are source-side identities after the documented coordinate,
/// helper-node, and inherit-mode normalization; they are not raw FBX object
/// transforms.
fn extract_source_skeleton(scene: &ufbx::Scene) -> SourceSkeletonAssets {
    let nodes = scene
        .nodes
        .iter()
        .map(|node| {
            let mut source = SourceNodeAsset::new(
                node.element.typed_id as usize,
                SourceNodeLocalRest::Trs {
                    translation: vec3(node.local_transform.translation),
                    rotation: quat(node.local_transform.rotation),
                    scale: vec3(node.local_transform.scale),
                },
            );
            source.name = (!node.element.name.is_empty()).then(|| node.element.name.to_string());
            source.parent_source_node_index = node
                .parent
                .as_ref()
                .map(|parent| parent.element.typed_id as usize);
            source.scene_root_indices = if node.is_root { vec![0] } else { Vec::new() };
            source.bone = Some(node.element.typed_id as usize);
            source
        })
        .collect();

    // A missing cluster bone removes a declared joint slot from the current
    // format-neutral shape: SourceSkinAsset has no optional/invalid joint-row
    // representation. Do not filter that slot and still claim complete source
    // coverage. The capability inventory retains the exact incomplete-cluster
    // count while the generic sidecar fails closed as globally unavailable.
    if scene.skin_clusters.iter().any(|cluster| {
        cluster.bone_node.as_ref().is_none_or(|bone| {
            usize::try_from(bone.element.typed_id)
                .ok()
                .is_none_or(|index| index >= scene.nodes.len())
        })
    }) {
        return SourceSkeletonAssets::default();
    }

    let mut attachments = vec![Vec::new(); scene.skin_deformers.len()];
    for node in &scene.nodes {
        let Some(mesh) = &node.mesh else { continue };
        for skin in &mesh.skin_deformers {
            let Some(for_skin) = attachments.get_mut(skin.element.typed_id as usize) else {
                return SourceSkeletonAssets::default();
            };
            for_skin.push(SourceSkinAttachment {
                source_node_index: node.element.typed_id as usize,
                source_mesh_index: Some(mesh.element.typed_id as usize),
            });
        }
    }

    let skins = scene
        .skin_deformers
        .iter()
        .map(|skin| {
            let source_skin_index = skin.element.typed_id as usize;
            let projected_matrices = skin
                .clusters
                .iter()
                .map(|cluster| project_cluster_bind(cluster).map(|(_, bind)| bind))
                .collect::<Option<Vec<_>>>();
            let (status, matrices) = match (skin.clusters.is_empty(), projected_matrices) {
                (true, _) => (SourceInverseBindAccessorStatus::Absent, Vec::new()),
                (false, Some(matrices)) => (SourceInverseBindAccessorStatus::Available, matrices),
                // Unreadable is declaration-wide because the generic shape
                // cannot retain a hole without shifting later joint slots.
                (false, None) => (SourceInverseBindAccessorStatus::Unreadable, Vec::new()),
            };
            SourceSkinAsset {
                source_skin_index,
                name: (!skin.element.name.is_empty()).then(|| skin.element.name.to_string()),
                // FBX skin deformers do not carry a glTF-style explicit
                // skeleton-root declaration. Do not infer one.
                skeleton_root_source_node_index: None,
                joint_source_node_indices: skin
                    .clusters
                    .iter()
                    .filter_map(|cluster| {
                        cluster
                            .bone_node
                            .as_ref()
                            .map(|node| node.element.typed_id as usize)
                    })
                    .collect(),
                inverse_bind_accessor: SourceInverseBindAccessor {
                    status,
                    declared_count: (!skin.clusters.is_empty()).then_some(skin.clusters.len()),
                    matrices,
                },
                attachments: attachments
                    .get_mut(source_skin_index)
                    .map(std::mem::take)
                    .unwrap_or_default(),
            }
        })
        .collect();

    SourceSkeletonAssets {
        coverage: SourceSkeletonCoverage::Complete,
        nodes,
        skins,
    }
}

/// Extract triangulated geometry, skins, and factor-only materials with
/// optional base-color and normal textures. Corner attributes come straight
/// from ufbx's indexed accessors; skin weights keep the top four influences
/// per source vertex and are renormalized.
fn extract_assets(
    scene: &ufbx::Scene,
    base_dir: Option<&Path>,
) -> (SceneAssets, AssetConversionFacts) {
    let mut assets = SceneAssets::default();
    let mut conversion = AssetConversionFacts::default();
    let mut material_index: std::collections::BTreeMap<u32, usize> =
        std::collections::BTreeMap::new();
    let mut normalized_mesh_index_by_source = std::collections::BTreeMap::<u32, usize>::new();

    for (source_node_index, node) in scene.nodes.iter().enumerate() {
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
                        let normal_texture = normal_texture(m, base_dir);
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
                            normal_texture,
                            metallic_roughness_texture: None,
                            occlusion_texture: None,
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
            .and_then(|s| {
                s.clusters
                    .iter()
                    .map(|cluster| project_cluster_bind(cluster).map(|(_, bind)| bind))
                    .collect::<Option<Vec<_>>>()
            })
            .unwrap_or_default();
        if let Some(&normalized_mesh_index) =
            normalized_mesh_index_by_source.get(&mesh.element.typed_id)
        {
            assets.instances.push(MeshInstance {
                source_node_index,
                node: node_id,
                mesh: normalized_mesh_index,
                skin_joints,
                skin_ibms,
            });
            continue;
        }
        let vertex_influences: Vec<Option<([u16; 4], [f32; 4])>> = skin
            .map(|s| {
                (0..mesh.num_vertices)
                    .map(|v| {
                        let mut pairs: Vec<(u16, f32)> = Vec::new();
                        if let Some(sv) = s.vertices.get(v) {
                            let begin = sv.weight_begin as usize;
                            let end = begin.saturating_add(sv.num_weights as usize);
                            for sw in s.weights.get(begin..end).unwrap_or_default() {
                                let source_weight = sw.weight;
                                let cluster_index = usize::try_from(sw.cluster_index).ok();
                                let cluster_has_bone = cluster_index
                                    .and_then(|index| s.clusters.get(index))
                                    .is_some_and(|cluster| cluster.bone_node.is_some());
                                match project_skin_influence(
                                    source_weight,
                                    cluster_index,
                                    s.clusters.len(),
                                    cluster_has_bone,
                                ) {
                                    ProjectedInfluence::Absent => {}
                                    ProjectedInfluence::Retained(index, weight) => {
                                        pairs.push((index, weight));
                                    }
                                    ProjectedInfluence::Rejected => {
                                        conversion.rejected_influence_count += 1;
                                    }
                                }
                            }
                        }
                        pairs.sort_by(|a, b| b.1.total_cmp(&a.1));
                        if pairs.len() > 4 {
                            conversion.truncated_influence_vertex_count += 1;
                            conversion.discarded_influence_count += pairs.len() - 4;
                        }
                        pairs.truncate(4);
                        let total: f32 = pairs.iter().map(|p| p.1).sum();
                        if pairs.is_empty() || !total.is_finite() || total <= 0.0 {
                            return None;
                        }
                        let mut joints = [0u16; 4];
                        let mut weights = [0f32; 4];
                        let mut renormalized = false;
                        for (slot, (j, w)) in pairs.into_iter().enumerate() {
                            joints[slot] = j;
                            let normalized = if total > 0.0 { w / total } else { 0.0 };
                            renormalized |= normalized.to_bits() != w.to_bits();
                            weights[slot] = normalized;
                        }
                        if renormalized {
                            conversion.renormalized_influence_vertex_count += 1;
                        }
                        Some((joints, weights))
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
                    let (joints, weights) = vertex_influences
                        .get(vertex)
                        .copied()
                        .flatten()
                        .unwrap_or_else(|| {
                            conversion.missing_skin_influence_corner_count += 1;
                            ([0; 4], [0.0; 4])
                        });
                    prim.joints.push(joints);
                    prim.weights.push(weights);
                }
            }
        }
        primitives.retain(|p| !p.positions.is_empty());
        for prim in &mut primitives {
            conversion.pre_weld_vertex_count += prim.positions.len();
            prim.weld();
            conversion.post_weld_vertex_count += prim.positions.len();
        }
        if primitives.is_empty() {
            continue;
        }
        let normalized_mesh_index = assets.meshes.len();
        let source_mesh_index = mesh.element.typed_id as usize;
        normalized_mesh_index_by_source.insert(mesh.element.typed_id, normalized_mesh_index);
        assets.meshes.push(MeshAsset {
            name: mesh.element.name.to_string(),
            // Retain the stable ufbx mesh identity even when an earlier
            // source definition emitted no normalized primitive. The compact
            // normalized vector index is owned independently by MeshInstance.
            source_mesh_index,
            primitives,
        });
        assets.instances.push(MeshInstance {
            source_node_index,
            node: node_id,
            mesh: normalized_mesh_index,
            skin_joints,
            skin_ibms,
        });
    }
    assets.scenes.push(SceneAsset {
        source_scene_index: 0,
        name: None,
        roots: scene
            .nodes
            .iter()
            .filter(|node| node.is_root)
            .map(|node| node.element.typed_id as usize)
            .collect(),
    });
    assets.default_scene = Some(0);
    assets.source_skeleton = extract_source_skeleton(scene);
    (assets, conversion)
}

#[cfg(test)]
mod tests {
    use super::{ProjectedInfluence, project_skin_influence};

    #[test]
    fn influence_projection_checks_sign_range_and_u16_cluster_narrowing() {
        assert_eq!(
            project_skin_influence(0.0, Some(0), 1, true),
            ProjectedInfluence::Absent
        );
        assert_eq!(
            project_skin_influence(-0.25, Some(0), 1, true),
            ProjectedInfluence::Rejected
        );
        assert_eq!(
            project_skin_influence(1.0, Some(1), 1, true),
            ProjectedInfluence::Rejected,
            "a source cluster index outside the declaration must not survive"
        );
        assert_eq!(
            project_skin_influence(
                1.0,
                Some(usize::from(u16::MAX) + 1),
                usize::from(u16::MAX) + 2,
                true,
            ),
            ProjectedInfluence::Rejected,
            "u32/usize cluster identity must not wrap while narrowing to u16"
        );
        assert_eq!(
            project_skin_influence(0.5, Some(7), 8, true),
            ProjectedInfluence::Retained(7, 0.5)
        );
    }
}
