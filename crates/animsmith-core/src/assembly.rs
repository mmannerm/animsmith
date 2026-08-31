//! Lossless, name-based clip operations for character-assembly pipelines.
//!
//! These helpers deliberately do not retarget motion.  They only move a clip
//! between skeletons with the same exact bone names, remove selected channels,
//! fill omitted channels from an authoritative rest pose, and make quaternion
//! signs consistent.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use glam::{Quat, Vec3};
use thiserror::Error;

use crate::model::{
    BoneId, Clip, Document, DocumentShapeError, Interpolation, Property, Skeleton,
    SourceSkeletonAssets, SourceSkeletonCoverage, Track, TrackValues, validate_document_shape,
};

/// Failure while resolving an exact bone name for an assembly operation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum AssemblyError {
    /// A track references a source bone outside its skeleton.
    #[error("track references source bone {bone}, but the source skeleton has {bone_count} bones")]
    SourceBoneOutOfBounds {
        /// Referenced source bone index.
        bone: BoneId,
        /// Number of source bones.
        bone_count: usize,
    },
    /// More than one source bone has a referenced exact name.
    #[error("source bone name {name:?} is ambiguous (bones {first} and {second})")]
    AmbiguousSourceName {
        /// Ambiguous name.
        name: String,
        /// First source bone with this name.
        first: BoneId,
        /// Another source bone with this name.
        second: BoneId,
    },
    /// More than one destination bone has a referenced exact name.
    #[error("base bone name {name:?} is ambiguous (bones {first} and {second})")]
    AmbiguousBaseName {
        /// Ambiguous name.
        name: String,
        /// First base bone with this name.
        first: BoneId,
        /// Another base bone with this name.
        second: BoneId,
    },
    /// A referenced source-bone name is absent from the base skeleton.
    #[error("source bone {source_bone} named {name:?} is missing from the base skeleton")]
    MissingBaseBone {
        /// Referenced source bone index.
        source_bone: BoneId,
        /// Missing exact name.
        name: String,
    },
    /// A supplied named-bone selection is ambiguous in the skeleton.
    #[error("bone name {name:?} is ambiguous (bones {first} and {second})")]
    AmbiguousSelectedName {
        /// Ambiguous name.
        name: String,
        /// First matching bone.
        first: BoneId,
        /// Another matching bone.
        second: BoneId,
    },
    /// A caller-selected completion target is outside the base skeleton.
    #[error("selected base bone {bone} is outside the base skeleton ({bone_count} bones)")]
    SelectedBoneOutOfBounds {
        /// Invalid selected bone.
        bone: BoneId,
        /// Number of base bones.
        bone_count: usize,
    },
    /// A requested node name is absent from the assembled skeleton.
    #[error("selected node name {name:?} is missing from the assembled skeleton")]
    MissingSelectedName {
        /// Missing exact node name.
        name: String,
    },
    /// A requested node name occurs more than once in the assembled skeleton.
    #[error("selected node name {name:?} is ambiguous (nodes {first} and {second})")]
    AmbiguousRemovalName {
        /// Ambiguous exact node name.
        name: String,
        /// First matching node.
        first: BoneId,
        /// Another matching node.
        second: BoneId,
    },
    /// Removing the selected closures would leave no skeleton nodes.
    #[error("selected node closures contain the entire assembled skeleton")]
    EntireSkeletonSelected,
    /// A final animation track still targets a selected closure.
    #[error("clip {clip_index} track {track_index} still targets selected node {bone}")]
    RemovalTrackReference {
        /// Final clip index.
        clip_index: usize,
        /// Final track index.
        track_index: usize,
        /// Referenced node.
        bone: BoneId,
    },
    /// A mesh instance is attached to a selected closure.
    #[error("mesh instance {instance_index} is attached to selected node {bone}")]
    RemovalMeshInstanceReference {
        /// Mesh-instance index.
        instance_index: usize,
        /// Referenced node.
        bone: BoneId,
    },
    /// A skin joint belongs to a selected closure.
    #[error(
        "mesh instance {instance_index} skin joint {joint_index} references selected node {bone}"
    )]
    RemovalSkinJointReference {
        /// Mesh-instance index.
        instance_index: usize,
        /// Joint slot.
        joint_index: usize,
        /// Referenced node.
        bone: BoneId,
    },
    /// A selected node carries bone-level inverse-bind evidence.
    #[error("selected node {bone} carries an inverse bind and is skin-referenced")]
    RemovalBoneInverseBindReference {
        /// Referenced node.
        bone: BoneId,
    },
    /// Complete source-skin evidence references a selected closure.
    #[error(
        "source skin {source_skin_index} references selected source node {source_node_index} projected to node {bone}"
    )]
    RemovalSourceSkinReference {
        /// Stable source-skin index.
        source_skin_index: usize,
        /// Stable source-node index.
        source_node_index: usize,
        /// Referenced normalized node.
        bone: BoneId,
    },
    /// The document no longer has the skeleton used to build a removal plan.
    #[error("node-removal plan does not match the assembled skeleton")]
    RemovalPlanDocumentMismatch,
    /// The strict input or projected document shape is invalid.
    #[error("node-removal document is invalid: {violation}")]
    InvalidRemovalDocument {
        /// Strict structural violation.
        violation: DocumentShapeError,
    },
}

/// One node in a planned subtree removal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedNode {
    /// Node index before structural projection.
    pub original_node_index: BoneId,
    /// Exact node name before structural projection.
    pub name: String,
    /// Parent index before structural projection.
    pub original_parent_node_index: Option<BoneId>,
    /// Whether the recipe selected this node directly rather than through an ancestor.
    pub selected: bool,
}

/// A validated, deterministic node-subtree projection plan.
#[derive(Debug, Clone)]
pub struct NodeSubtreeRemovalPlan {
    skeleton_identity: Vec<(String, Option<BoneId>)>,
    old_to_new: Vec<Option<BoneId>>,
    removed_nodes: Vec<RemovedNode>,
}

impl NodeSubtreeRemovalPlan {
    /// Whether `bone` belongs to a selected subtree.
    #[must_use]
    pub fn removes(&self, bone: BoneId) -> bool {
        self.old_to_new.get(bone).is_some_and(Option::is_none)
    }

    /// Removed nodes in original parent-before-child order.
    #[must_use]
    pub fn removed_nodes(&self) -> &[RemovedNode] {
        &self.removed_nodes
    }
}

/// Resolve exact node selectors and their complete descendant closure.
///
/// The plan is non-mutating. Call [`apply_node_subtree_removal`] after all
/// animation transforms have finished; it revalidates every live reference
/// before committing the projection.
///
/// # Errors
///
/// Returns [`AssemblyError`] for an invalid document, missing or ambiguous
/// selector, or a selection that contains the entire skeleton.
pub fn plan_node_subtree_removal(
    document: &Document,
    names: &[String],
) -> Result<NodeSubtreeRemovalPlan, AssemblyError> {
    validate_document_shape(document)
        .map_err(|violation| AssemblyError::InvalidRemovalDocument { violation })?;

    let mut selected = vec![false; document.skeleton.bones.len()];
    let mut matches = names
        .iter()
        .map(|name| (name.as_str(), Vec::new()))
        .collect::<HashMap<_, _>>();
    for (bone, candidate) in document.skeleton.bones.iter().enumerate() {
        if let Some(matches) = matches.get_mut(candidate.name.as_str()) {
            matches.push(bone);
        }
    }
    for name in names {
        let matches = &matches[name.as_str()];
        let Some(&first) = matches.first() else {
            return Err(AssemblyError::MissingSelectedName { name: name.clone() });
        };
        if let Some(&second) = matches.get(1) {
            return Err(AssemblyError::AmbiguousRemovalName {
                name: name.clone(),
                first,
                second,
            });
        }
        selected[first] = true;
    }

    let mut removed = vec![false; document.skeleton.bones.len()];
    for (bone, entry) in document.skeleton.bones.iter().enumerate() {
        removed[bone] = selected[bone] || entry.parent.is_some_and(|parent| removed[parent]);
    }
    if !removed.is_empty() && removed.iter().all(|removed| *removed) {
        return Err(AssemblyError::EntireSkeletonSelected);
    }

    let mut next = 0;
    let old_to_new = removed
        .iter()
        .map(|removed| {
            if *removed {
                None
            } else {
                let mapped = next;
                next += 1;
                Some(mapped)
            }
        })
        .collect();
    let removed_nodes = document
        .skeleton
        .bones
        .iter()
        .enumerate()
        .filter(|(bone, _)| removed[*bone])
        .map(|(bone, entry)| RemovedNode {
            original_node_index: bone,
            name: entry.name.clone(),
            original_parent_node_index: entry.parent,
            selected: selected[bone],
        })
        .collect();
    Ok(NodeSubtreeRemovalPlan {
        skeleton_identity: document
            .skeleton
            .bones
            .iter()
            .map(|bone| (bone.name.clone(), bone.parent))
            .collect(),
        old_to_new,
        removed_nodes,
    })
}

/// Apply a planned subtree projection transactionally.
///
/// All final track, mesh-instance, and skin references are checked before the
/// document is changed. Source-native skeleton projection is cleared:
/// after authored node deletion it cannot remain a complete account of the
/// new normalized hierarchy.
///
/// # Errors
///
/// Returns [`AssemblyError`] if the plan is stale, a forbidden reference
/// survives, or the projected document is structurally invalid. `document`
/// is unchanged on every error.
pub fn apply_node_subtree_removal(
    document: &mut Document,
    plan: &NodeSubtreeRemovalPlan,
) -> Result<(), AssemblyError> {
    let identity = document
        .skeleton
        .bones
        .iter()
        .map(|bone| (bone.name.clone(), bone.parent))
        .collect::<Vec<_>>();
    if identity != plan.skeleton_identity {
        return Err(AssemblyError::RemovalPlanDocumentMismatch);
    }
    validate_document_shape(document)
        .map_err(|violation| AssemblyError::InvalidRemovalDocument { violation })?;
    if plan.removed_nodes.is_empty() {
        return Ok(());
    }

    for (clip_index, clip) in document.clips.iter().enumerate() {
        for (track_index, track) in clip.tracks.iter().enumerate() {
            if plan.removes(track.bone) {
                return Err(AssemblyError::RemovalTrackReference {
                    clip_index,
                    track_index,
                    bone: track.bone,
                });
            }
        }
    }
    for (instance_index, instance) in document.assets.instances.iter().enumerate() {
        if plan.removes(instance.node) {
            return Err(AssemblyError::RemovalMeshInstanceReference {
                instance_index,
                bone: instance.node,
            });
        }
        for (joint_index, &bone) in instance.skin_joints.iter().enumerate() {
            if plan.removes(bone) {
                return Err(AssemblyError::RemovalSkinJointReference {
                    instance_index,
                    joint_index,
                    bone,
                });
            }
        }
    }
    for (bone, entry) in document.skeleton.bones.iter().enumerate() {
        if plan.removes(bone) && entry.inverse_bind.is_some() {
            return Err(AssemblyError::RemovalBoneInverseBindReference { bone });
        }
    }
    if document.assets.source_skeleton.coverage == SourceSkeletonCoverage::Complete {
        let projected = document
            .assets
            .source_skeleton
            .nodes
            .iter()
            .filter_map(|node| node.bone.map(|bone| (node.source_node_index, bone)))
            .collect::<HashMap<_, _>>();
        for skin in &document.assets.source_skeleton.skins {
            let source_nodes = skin
                .joint_source_node_indices
                .iter()
                .copied()
                .chain(skin.skeleton_root_source_node_index)
                .chain(
                    skin.attachments
                        .iter()
                        .map(|attachment| attachment.source_node_index),
                );
            for source_node_index in source_nodes {
                if let Some(&bone) = projected.get(&source_node_index)
                    && plan.removes(bone)
                {
                    return Err(AssemblyError::RemovalSourceSkinReference {
                        source_skin_index: skin.source_skin_index,
                        source_node_index,
                        bone,
                    });
                }
            }
        }
    }

    document.skeleton.bones = std::mem::take(&mut document.skeleton.bones)
        .into_iter()
        .enumerate()
        .filter_map(|(bone, mut entry)| {
            plan.old_to_new[bone]?;
            entry.parent = entry.parent.map(|parent| {
                plan.old_to_new[parent]
                    .expect("a retained node cannot have a removed ancestor outside its closure")
            });
            Some(entry)
        })
        .collect();
    for clip in &mut document.clips {
        for track in &mut clip.tracks {
            track.bone = plan.old_to_new[track.bone]
                .expect("live track references were rejected before projection");
        }
    }
    for instance in &mut document.assets.instances {
        instance.node = plan.old_to_new[instance.node]
            .expect("live instance references were rejected before projection");
        for bone in &mut instance.skin_joints {
            *bone = plan.old_to_new[*bone]
                .expect("live joint references were rejected before projection");
        }
    }
    for scene in &mut document.assets.scenes {
        scene.roots = scene
            .roots
            .iter()
            .filter_map(|&root| plan.old_to_new[root])
            .collect();
    }
    document.assets.source_skeleton = SourceSkeletonAssets::default();
    debug_assert!(validate_document_shape(document).is_ok());
    Ok(())
}

/// Selects which absent local-transform channels [`complete_rest_pose_tracks`]
/// creates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RestPoseTrackOptions {
    /// Complete absent translation channels.
    pub translation: bool,
    /// Complete absent rotation channels.
    pub rotation: bool,
    /// Complete absent scale channels.
    pub scale: bool,
}

impl RestPoseTrackOptions {
    /// Select all three local-transform channels.
    pub const ALL: Self = Self {
        translation: true,
        rotation: true,
        scale: true,
    };
}

/// Remap a source clip onto `base` using exact, unique bone names.
///
/// The returned clip has the same keys, values, interpolation, name, and
/// duration as `source`; only each track's bone index changes.  Every source
/// bone referenced by a track and its base counterpart must have a unique,
/// exact name.  Names outside the referenced set are intentionally irrelevant.
///
/// # Errors
///
/// Returns [`AssemblyError`] if a track references an invalid source bone, or
/// if a referenced source name is ambiguous or missing in `base`.
pub fn remap_clip_to_base(
    source: &Clip,
    source_skeleton: &Skeleton,
    base: &Skeleton,
) -> Result<Clip, AssemblyError> {
    let referenced: BTreeSet<BoneId> = source.tracks.iter().map(|track| track.bone).collect();
    let mut referenced_names = BTreeSet::new();
    for &bone in &referenced {
        let Some(source_bone) = source_skeleton.bones.get(bone) else {
            return Err(AssemblyError::SourceBoneOutOfBounds {
                bone,
                bone_count: source_skeleton.bones.len(),
            });
        };
        referenced_names.insert(source_bone.name.as_str());
    }
    let mut source_names = BTreeMap::new();
    for (bone, source_bone) in source_skeleton.bones.iter().enumerate() {
        if !referenced_names.contains(source_bone.name.as_str()) {
            continue;
        }
        if let Some(first) = source_names.insert(source_bone.name.as_str(), bone) {
            return Err(AssemblyError::AmbiguousSourceName {
                name: source_bone.name.clone(),
                first,
                second: bone,
            });
        }
    }

    let mut base_names = BTreeMap::new();
    for (bone, base_bone) in base.bones.iter().enumerate() {
        if !referenced_names.contains(base_bone.name.as_str()) {
            continue;
        }
        if let Some(first) = base_names.insert(base_bone.name.as_str(), bone) {
            return Err(AssemblyError::AmbiguousBaseName {
                name: base_bone.name.clone(),
                first,
                second: bone,
            });
        }
    }

    let remapped: BTreeMap<BoneId, BoneId> = referenced
        .iter()
        .map(|&source_bone| {
            let name = source_skeleton.bones[source_bone].name.as_str();
            base_names
                .get(name)
                .copied()
                .map(|base_bone| (source_bone, base_bone))
                .ok_or_else(|| AssemblyError::MissingBaseBone {
                    source_bone,
                    name: name.to_owned(),
                })
        })
        .collect::<Result<_, _>>()?;

    let mut output = source.clone();
    for track in &mut output.tracks {
        track.bone = remapped[&track.bone];
    }
    Ok(output)
}

/// Remove every track targeting any exact, uniquely named `bones` entry.
///
/// Unknown names select no tracks.  A duplicated selected name is rejected so
/// a caller cannot silently strip motion from an unintended bone.
///
/// # Errors
///
/// Returns [`AssemblyError::AmbiguousSelectedName`] if a requested name occurs
/// more than once in `skeleton`.
pub fn strip_named_bone_tracks(
    clip: &mut Clip,
    skeleton: &Skeleton,
    bones: impl IntoIterator<Item = impl AsRef<str>>,
) -> Result<usize, AssemblyError> {
    let requested: BTreeSet<String> = bones
        .into_iter()
        .map(|name| name.as_ref().to_owned())
        .collect();
    let mut selected = BTreeSet::new();
    for name in requested {
        let matches: Vec<_> = skeleton
            .bones
            .iter()
            .enumerate()
            .filter_map(|(id, bone)| (bone.name == name).then_some(id))
            .collect();
        if let [bone] = matches.as_slice() {
            selected.insert(*bone);
        } else if let [first, second, ..] = matches.as_slice() {
            return Err(AssemblyError::AmbiguousSelectedName {
                name,
                first: *first,
                second: *second,
            });
        }
    }
    let before = clip.tracks.len();
    clip.tracks.retain(|track| !selected.contains(&track.bone));
    Ok(before - clip.tracks.len())
}

/// Add one rest-pose key at time zero for selected channels that a clip omits.
///
/// Existing tracks, including empty ones, are never altered.  The created
/// tracks use linear interpolation; one key is a constant hold under the core
/// sampler.  Tracks are appended in skeleton order, then translation,
/// rotation, scale order for deterministic output.
pub fn complete_rest_pose_tracks(
    clip: &mut Clip,
    base: &Skeleton,
    options: RestPoseTrackOptions,
) -> Result<usize, AssemblyError> {
    complete_rest_pose_tracks_for_bones(clip, base, 0..base.bones.len(), options)
}

/// Add absent rest-pose channels for an explicit base-bone selection.
///
/// Bone ids are sorted and deduplicated before tracks are appended, so caller
/// ordering cannot change the output. Existing tracks are never altered.
///
/// # Errors
///
/// Returns [`AssemblyError::SelectedBoneOutOfBounds`] when a selected id is not
/// present in `base`.
pub fn complete_rest_pose_tracks_for_bones(
    clip: &mut Clip,
    base: &Skeleton,
    bones: impl IntoIterator<Item = BoneId>,
    options: RestPoseTrackOptions,
) -> Result<usize, AssemblyError> {
    let properties = [
        (Property::Translation, options.translation),
        (Property::Rotation, options.rotation),
        (Property::Scale, options.scale),
    ];
    let mut added = 0;
    let bones = bones.into_iter().collect::<BTreeSet<_>>();
    for bone in bones {
        let Some(base_bone) = base.bones.get(bone) else {
            return Err(AssemblyError::SelectedBoneOutOfBounds {
                bone,
                bone_count: base.bones.len(),
            });
        };
        for (property, enabled) in properties {
            if !enabled
                || clip
                    .tracks
                    .iter()
                    .any(|track| track.bone == bone && track.property == property)
            {
                continue;
            }
            clip.tracks.push(rest_track(
                bone,
                property,
                base_bone.rest.translation,
                base_bone.rest.rotation,
                base_bone.rest.scale,
            ));
            added += 1;
        }
    }
    Ok(added)
}

fn rest_track(
    bone: BoneId,
    property: Property,
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
) -> Track {
    let values = match property {
        Property::Translation => TrackValues::Vec3s(vec![translation]),
        Property::Rotation => TrackValues::Quats(vec![rotation]),
        Property::Scale => TrackValues::Vec3s(vec![scale]),
    };
    Track {
        bone,
        property,
        interpolation: Interpolation::Linear,
        times: vec![0.0],
        values,
    }
}

/// Make rotation-track key quaternions hemisphere-consistent in key order.
///
/// Each finite key is compared to the preceding finite key.  If their dot
/// product is negative, the key is negated.  Cubic-spline keys negate their
/// whole tangent/value/tangent triplet, preserving the represented curve.
/// A zero dot product is deliberately left unchanged, making ties stable and
/// avoiding platform-dependent sign choices.  Returns the number of flipped
/// keys.
pub fn normalize_quaternion_hemispheres(clip: &mut Clip) -> usize {
    let mut flipped = 0;
    for track in &mut clip.tracks {
        if track.property != Property::Rotation {
            continue;
        }
        let key_count = track.key_count();
        let interpolation = track.interpolation;
        let TrackValues::Quats(values) = &mut track.values else {
            continue;
        };
        let mut previous = None;
        for key in 0..key_count {
            let value_index = match interpolation {
                Interpolation::CubicSpline => key * 3 + 1,
                _ => key,
            };
            let Some(value) = values.get(value_index).copied() else {
                break;
            };
            if !value.is_finite() {
                continue;
            }
            if previous.is_some_and(|previous: Quat| previous.dot(value) < 0.0) {
                let start = match interpolation {
                    Interpolation::CubicSpline => key * 3,
                    _ => key,
                };
                let count = match interpolation {
                    Interpolation::CubicSpline => 3,
                    _ => 1,
                };
                if start + count > values.len() {
                    break;
                }
                for quaternion in &mut values[start..start + count] {
                    *quaternion = -*quaternion;
                }
                previous = Some(-value);
                flipped += 1;
            } else {
                previous = Some(value);
            }
        }
    }
    flipped
}

/// Remove one final key from every nonempty track and shorten the clip to the
/// latest remaining key.
///
/// This is useful for converting a duplicate-endpoint loop to an open loop.
/// Empty tracks are removed.  If every track becomes empty, the duration is
/// zero.  Cubic-spline tangent/value/tangent triplets are removed together.
/// Returns the number of keys removed.
pub fn remove_final_keys(clip: &mut Clip) -> usize {
    let mut removed = 0;
    for track in &mut clip.tracks {
        if track.times.pop().is_none() {
            continue;
        }
        removed += 1;
        let count = match track.interpolation {
            Interpolation::CubicSpline => 3,
            _ => 1,
        };
        match &mut track.values {
            TrackValues::Vec3s(values) => values.truncate(values.len().saturating_sub(count)),
            TrackValues::Quats(values) => values.truncate(values.len().saturating_sub(count)),
        }
    }
    clip.tracks.retain(|track| !track.times.is_empty());
    clip.duration_s = clip
        .tracks
        .iter()
        .map(|track| track.end_time() as f64)
        .fold(0.0, f64::max);
    removed
}

#[cfg(test)]
mod node_subtree_removal_tests {
    use super::*;
    use crate::model::{
        Bone, MaterialAsset, MeshAsset, MeshInstance, SceneAsset, SceneAssets, SourceNodeAsset,
        SourceNodeLocalRest, SourceSkinAsset, TextureAsset, Transform,
    };

    fn bone(name: &str, parent: Option<BoneId>) -> Bone {
        Bone {
            name: name.into(),
            parent,
            rest: Transform::IDENTITY,
            inverse_bind: None,
        }
    }

    fn source_node(
        source_node_index: usize,
        bone: BoneId,
        parent: Option<usize>,
    ) -> SourceNodeAsset {
        let mut node = SourceNodeAsset::new(
            source_node_index,
            SourceNodeLocalRest::Trs {
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            },
        );
        node.parent_source_node_index = parent;
        node.bone = Some(bone);
        node
    }

    fn fixture() -> Document {
        let bones = vec![
            bone("kept-root-a", None),
            bone("prop-root", None),
            bone("prop-child", Some(1)),
            bone("kept-root-b", None),
            bone("kept-child", Some(3)),
        ];
        let source_skeleton = SourceSkeletonAssets {
            coverage: SourceSkeletonCoverage::Complete,
            nodes: vec![
                source_node(100, 0, None),
                source_node(101, 1, None),
                source_node(102, 2, Some(101)),
                source_node(103, 3, None),
                source_node(104, 4, Some(103)),
            ],
            skins: vec![SourceSkinAsset {
                source_skin_index: 7,
                skeleton_root_source_node_index: Some(103),
                joint_source_node_indices: vec![100, 104],
                ..SourceSkinAsset::default()
            }],
        };
        Document {
            skeleton: Skeleton { bones },
            clips: vec![Clip {
                name: "walk".into(),
                duration_s: 1.0,
                tracks: vec![Track {
                    bone: 4,
                    property: Property::Translation,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0],
                    values: TrackValues::Vec3s(vec![Vec3::ZERO]),
                }],
            }],
            assets: SceneAssets {
                meshes: vec![MeshAsset {
                    name: "kept-mesh".into(),
                    ..MeshAsset::default()
                }],
                materials: vec![MaterialAsset {
                    name: "preexisting-orphan-material".into(),
                    base_color: [1.0; 4],
                    metallic: 0.0,
                    roughness: 1.0,
                    base_color_texture: Some(TextureAsset {
                        bytes: vec![1, 2, 3, 4],
                        mime: "image/png".into(),
                    }),
                    normal_texture: None,
                    metallic_roughness_texture: None,
                    occlusion_texture: None,
                }],
                instances: vec![MeshInstance {
                    source_node_index: 103,
                    node: 3,
                    mesh: 0,
                    skin_joints: vec![0, 4],
                    ..MeshInstance::default()
                }],
                scenes: vec![SceneAsset {
                    source_scene_index: 4,
                    name: Some("scene".into()),
                    roots: vec![0, 1, 3],
                }],
                source_skeleton,
                ..SceneAssets::default()
            },
            ..Document::default()
        }
    }

    fn plan_prop(document: &Document) -> NodeSubtreeRemovalPlan {
        plan_node_subtree_removal(document, &["prop-root".into()]).expect("plan prop subtree")
    }

    fn assert_unchanged(document: &Document, before: &str) {
        assert_eq!(format!("{document:?}"), before);
    }

    #[test]
    fn plan_resolves_exact_unique_names_and_marks_complete_closure() {
        let document = fixture();
        let plan = plan_node_subtree_removal(&document, &["prop-root".into(), "prop-child".into()])
            .unwrap();

        assert!(!plan.removes(0));
        assert!(plan.removes(1));
        assert!(plan.removes(2));
        assert!(!plan.removes(3));
        assert!(!plan.removes(99));
        assert_eq!(
            plan.removed_nodes(),
            [
                RemovedNode {
                    original_node_index: 1,
                    name: "prop-root".into(),
                    original_parent_node_index: None,
                    selected: true,
                },
                RemovedNode {
                    original_node_index: 2,
                    name: "prop-child".into(),
                    original_parent_node_index: Some(1),
                    selected: true,
                },
            ]
        );
        let reversed =
            plan_node_subtree_removal(&document, &["prop-child".into(), "prop-root".into()])
                .unwrap();
        assert_eq!(reversed.removed_nodes(), plan.removed_nodes());
    }

    #[test]
    fn plan_rejects_missing_ambiguous_and_entire_skeleton_selections() {
        let document = fixture();
        assert_eq!(
            plan_node_subtree_removal(&document, &["Prop-Root".into()]).unwrap_err(),
            AssemblyError::MissingSelectedName {
                name: "Prop-Root".into()
            }
        );

        let mut ambiguous = document.clone();
        ambiguous.skeleton.bones[3].name = "prop-root".into();
        assert_eq!(
            plan_node_subtree_removal(&ambiguous, &["prop-root".into()]).unwrap_err(),
            AssemblyError::AmbiguousRemovalName {
                name: "prop-root".into(),
                first: 1,
                second: 3,
            }
        );

        assert_eq!(
            plan_node_subtree_removal(
                &document,
                &[
                    "kept-root-a".into(),
                    "prop-root".into(),
                    "kept-root-b".into()
                ]
            )
            .unwrap_err(),
            AssemblyError::EntireSkeletonSelected
        );
    }

    #[test]
    fn apply_stably_remaps_every_normalized_bone_reference_and_clears_source_projection() {
        let mut document = fixture();
        let retained_inverse_bind = glam::Mat4::from_translation(Vec3::new(7.0, 8.0, 9.0));
        let retained_material = format!("{:?}", document.assets.materials[0]);
        document.skeleton.bones[4].inverse_bind = Some(retained_inverse_bind);
        let plan = plan_prop(&document);

        apply_node_subtree_removal(&mut document, &plan).unwrap();

        assert_eq!(
            document
                .skeleton
                .bones
                .iter()
                .map(|bone| (bone.name.as_str(), bone.parent))
                .collect::<Vec<_>>(),
            [
                ("kept-root-a", None),
                ("kept-root-b", None),
                ("kept-child", Some(1)),
            ]
        );
        assert_eq!(document.clips[0].tracks[0].bone, 2);
        assert_eq!(
            document.skeleton.bones[2].inverse_bind,
            Some(retained_inverse_bind)
        );
        assert_eq!(document.assets.instances[0].node, 1);
        assert_eq!(document.assets.instances[0].skin_joints, [0, 2]);
        assert_eq!(document.assets.scenes[0].roots, [0, 1]);
        assert_eq!(document.assets.meshes[0].name, "kept-mesh");
        assert_eq!(document.assets.materials.len(), 1);
        assert_eq!(
            format!("{:?}", document.assets.materials[0]),
            retained_material
        );
        assert!(
            document.assets.source_skeleton.coverage == SourceSkeletonCoverage::Unavailable
                && document.assets.source_skeleton.nodes.is_empty()
                && document.assets.source_skeleton.skins.is_empty()
        );
        validate_document_shape(&document).unwrap();
    }

    #[test]
    fn apply_refuses_surviving_track_target_transactionally() {
        let mut document = fixture();
        let plan = plan_prop(&document);
        document.clips[0].tracks[0].bone = 2;
        let before = format!("{document:?}");

        assert_eq!(
            apply_node_subtree_removal(&mut document, &plan).unwrap_err(),
            AssemblyError::RemovalTrackReference {
                clip_index: 0,
                track_index: 0,
                bone: 2,
            }
        );
        assert_unchanged(&document, &before);
    }

    #[test]
    fn apply_refuses_mesh_attachment_and_skin_joint_transactionally() {
        let mut attached = fixture();
        let plan = plan_prop(&attached);
        attached.assets.instances[0].node = 2;
        let before = format!("{attached:?}");
        assert_eq!(
            apply_node_subtree_removal(&mut attached, &plan).unwrap_err(),
            AssemblyError::RemovalMeshInstanceReference {
                instance_index: 0,
                bone: 2,
            }
        );
        assert_unchanged(&attached, &before);

        let mut skinned = fixture();
        let plan = plan_prop(&skinned);
        skinned.assets.instances[0].skin_joints[1] = 2;
        let before = format!("{skinned:?}");
        assert_eq!(
            apply_node_subtree_removal(&mut skinned, &plan).unwrap_err(),
            AssemblyError::RemovalSkinJointReference {
                instance_index: 0,
                joint_index: 1,
                bone: 2,
            }
        );
        assert_unchanged(&skinned, &before);
    }

    #[test]
    fn apply_refuses_every_complete_source_skin_reference() {
        for reference in ["joint", "root", "attachment"] {
            let mut document = fixture();
            let plan = plan_prop(&document);
            let skin = &mut document.assets.source_skeleton.skins[0];
            let source_node_index = match reference {
                "joint" => {
                    skin.joint_source_node_indices.push(102);
                    102
                }
                "root" => {
                    skin.skeleton_root_source_node_index = Some(101);
                    101
                }
                "attachment" => {
                    skin.attachments.push(crate::model::SourceSkinAttachment {
                        source_node_index: 102,
                        source_mesh_index: None,
                    });
                    102
                }
                _ => unreachable!(),
            };
            let before = format!("{document:?}");

            assert_eq!(
                apply_node_subtree_removal(&mut document, &plan).unwrap_err(),
                AssemblyError::RemovalSourceSkinReference {
                    source_skin_index: 7,
                    source_node_index,
                    bone: if source_node_index == 101 { 1 } else { 2 },
                }
            );
            assert_unchanged(&document, &before);
        }
    }

    #[test]
    fn apply_refuses_bone_inverse_bind_without_an_instance_or_source_projection() {
        let mut document = fixture();
        document.assets.instances.clear();
        document.assets.source_skeleton = SourceSkeletonAssets::default();
        document.skeleton.bones[2].inverse_bind = Some(glam::Mat4::IDENTITY);
        let plan = plan_prop(&document);
        let before = format!("{document:?}");

        assert_eq!(
            apply_node_subtree_removal(&mut document, &plan).unwrap_err(),
            AssemblyError::RemovalBoneInverseBindReference { bone: 2 }
        );
        assert_unchanged(&document, &before);
    }

    #[test]
    fn apply_checks_every_clip_and_property_for_surviving_track_targets() {
        for (property, values) in [
            (Property::Scale, TrackValues::Vec3s(vec![Vec3::ONE])),
            (
                Property::Rotation,
                TrackValues::Quats(vec![glam::Quat::IDENTITY]),
            ),
        ] {
            let mut document = fixture();
            document.clips.push(Clip {
                name: "second".into(),
                duration_s: 0.0,
                tracks: vec![Track {
                    bone: 2,
                    property,
                    interpolation: Interpolation::Linear,
                    times: vec![0.0],
                    values,
                }],
            });
            let plan = plan_prop(&document);
            let before = format!("{document:?}");
            assert_eq!(
                apply_node_subtree_removal(&mut document, &plan).unwrap_err(),
                AssemblyError::RemovalTrackReference {
                    clip_index: 1,
                    track_index: 0,
                    bone: 2,
                },
                "property {property:?}"
            );
            assert_unchanged(&document, &before);
        }
    }

    #[test]
    fn apply_revalidates_plan_identity_and_structural_references() {
        let mut renamed = fixture();
        let plan = plan_prop(&renamed);
        renamed.skeleton.bones[4].name = "changed-after-planning".into();
        let before = format!("{renamed:?}");
        assert_eq!(
            apply_node_subtree_removal(&mut renamed, &plan).unwrap_err(),
            AssemblyError::RemovalPlanDocumentMismatch
        );
        assert_unchanged(&renamed, &before);

        let mut invalid_scene = fixture();
        let plan = plan_prop(&invalid_scene);
        invalid_scene.assets.scenes[0].roots[2] = 99;
        let before = format!("{invalid_scene:?}");
        assert!(matches!(
            plan_node_subtree_removal(&invalid_scene, &["prop-root".into()]),
            Err(AssemblyError::InvalidRemovalDocument {
                violation: DocumentShapeError::SceneRootOutOfBounds { .. }
            })
        ));
        assert_eq!(
            apply_node_subtree_removal(&mut invalid_scene, &plan).unwrap_err(),
            AssemblyError::InvalidRemovalDocument {
                violation: DocumentShapeError::SceneRootOutOfBounds {
                    scene_index: 0,
                    root_index: 2,
                    bone: 99,
                }
            }
        );
        assert_unchanged(&invalid_scene, &before);
    }

    #[test]
    fn empty_plan_is_a_true_no_op_and_invalid_inputs_fail_closed() {
        let mut document = fixture();
        let before = format!("{document:?}");
        let plan = plan_node_subtree_removal(&document, &[]).unwrap();
        assert!(plan.removed_nodes().is_empty());
        apply_node_subtree_removal(&mut document, &plan).unwrap();
        assert_unchanged(&document, &before);

        let mut malformed = fixture();
        malformed.skeleton.bones[4].parent = Some(4);
        assert!(matches!(
            plan_node_subtree_removal(&malformed, &["prop-root".into()]),
            Err(AssemblyError::InvalidRemovalDocument { .. })
        ));
    }
}
