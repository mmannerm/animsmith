//! FBX ingestion into the animsmith core model, via the official `ufbx`
//! bindings (which bundle and build the single-file C library — no
//! system dependencies).
//!
//! Normalization happens at parse time through `LoadOpts`: glTF axis
//! conventions (right-handed, +Y up), metres, transform-adjust space
//! conversion (geometry untouched), helper nodes for 3ds Max geometric
//! transforms. The core model therefore only ever sees glTF-convention
//! data regardless of the source.
//!
//! Animation is extracted with ufbx's `bake_anim`, which evaluates anim
//! stacks/layers, cubic/TCB curves, pre/post-rotation, and
//! inherit-scale modes into resampled linear TRS keyframes — this
//! sidesteps FBX curve semantics entirely. Each anim stack (take)
//! becomes one core `Clip` with times shifted to start at 0.

use animsmith_core::model::{
    Bone, Clip, Document, Interpolation, Property, Skeleton, SourceInfo, Track, TrackValues,
    Transform,
};
use glam::{Mat4, Quat, Vec3};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("path is not valid UTF-8: {0}")]
    Path(String),
    #[error("FBX parse error: {0}")]
    Fbx(String),
    #[error("animation bake failed for take {take:?}: {message}")]
    Bake { take: String, message: String },
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

/// Load an `.fbx` file into a core [`Document`].
pub fn load(path: &Path) -> Result<Document, LoadError> {
    let filename = path
        .to_str()
        .ok_or_else(|| LoadError::Path(path.display().to_string()))?;
    let opts = ufbx::LoadOpts {
        target_axes: ufbx::CoordinateAxes::right_handed_y_up(),
        target_unit_meters: 1.0,
        space_conversion: ufbx::SpaceConversion::AdjustTransforms,
        geometry_transform_handling: ufbx::GeometryTransformHandling::HelperNodes,
        // Geometry isn't modelled yet; skip the heavy parts but keep
        // skins so inverse binds resolve.
        skip_mesh_parts: true,
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
                bones[id].inverse_bind = Some(mat4(&cluster.geometry_to_bone));
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

    Ok(Document {
        skeleton: Skeleton { bones },
        clips,
        source: SourceInfo {
            path: Some(path.display().to_string()),
            format: Some("fbx".into()),
        },
    })
}
