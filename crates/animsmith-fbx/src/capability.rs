//! Conservative ufbx-side scale capability inventory.

use animsmith_core::Document;
use animsmith_core::scale::{ScaleCapabilityCoverage, ScaleCapabilityFacts};

/// How one Appendix D.4 domain reaches the normalized FBX document.
///
/// These values describe semantic ingestion only. None claims raw FBX byte,
/// object-property, curve-key, or payload-span preservation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FbxScaleDomainStatus {
    /// The source inspection proved that the domain is absent.
    Absent,
    /// ufbx normalized the source representation before it reached the core model.
    Normalized,
    /// ufbx evaluated source animation into resampled linear TRS tracks.
    Baked,
    /// The value is derived from another normalized domain.
    Derived,
    /// The loader rebuilt the domain into a different normalized representation.
    Rebuilt,
    /// The source domain is present but not completely represented.
    Unsupported,
    /// ufbx exposes no raw-span relationship with which to prove this domain.
    Unverifiable,
}

/// Explicit status for every current domain row in DESIGN.md Appendix D.4.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct FbxScaleDomainInventory {
    /// Rest hierarchy and local transforms.
    pub rest_hierarchy: FbxScaleDomainStatus,
    /// Translation animation values and tangents.
    pub translation_animation: FbxScaleDomainStatus,
    /// Rotation and scale animation values and tangents.
    pub rotation_and_scale_animation: FbxScaleDomainStatus,
    /// Root-motion and velocity evidence derived from translation tracks.
    pub root_motion_and_velocity: FbxScaleDomainStatus,
    /// Base mesh positions and normals.
    pub base_mesh_geometry: FbxScaleDomainStatus,
    /// Morph targets and morph-weight animation.
    pub morphs: FbxScaleDomainStatus,
    /// Per-skin inverse-bind matrices.
    pub skin_binds: FbxScaleDomainStatus,
    /// Cameras and lights.
    pub cameras_and_lights: FbxScaleDomainStatus,
    /// Collision, custom properties, constraints, and unknown elements.
    pub collision_and_custom_data: FbxScaleDomainStatus,
    /// Other vertex attributes, deformers, and source geometry kinds.
    pub other_vertex_and_source_data: FbxScaleDomainStatus,
    /// Source transform-stack state outside the normalized TRS model.
    pub out_of_contract_node_transforms: FbxScaleDomainStatus,
    /// Animation targeting source transform-stack or matrix state.
    pub animation_targeting_matrix_nodes: FbxScaleDomainStatus,
    /// Shared raw payload spans corresponding to glTF accessors.
    pub shared_raw_accessor_payloads: FbxScaleDomainStatus,
    /// Raw payload spans corresponding to unreferenced glTF accessors.
    pub unreferenced_accessor_payloads: FbxScaleDomainStatus,
    /// Image payload spans that could alias scale-bearing source bytes.
    pub image_payload_aliases: FbxScaleDomainStatus,
}

impl FbxScaleDomainInventory {
    /// Every Appendix D.4 row in table order with its current FBX status.
    ///
    /// This is the mechanical bridge between the public named fields and the
    /// design table. Tests compare these names with the table so adding a row
    /// to either authority cannot silently leave the other incomplete.
    pub fn named_rows(&self) -> [(&'static str, FbxScaleDomainStatus); 15] {
        [
            ("Rest hierarchy", self.rest_hierarchy),
            ("Translation animation", self.translation_animation),
            (
                "Rotation and scale animation",
                self.rotation_and_scale_animation,
            ),
            ("Root motion and velocity", self.root_motion_and_velocity),
            ("Base mesh geometry", self.base_mesh_geometry),
            ("Morphs", self.morphs),
            ("Skin binds", self.skin_binds),
            ("Cameras/lights", self.cameras_and_lights),
            ("Collision/custom data", self.collision_and_custom_data),
            (
                "Other vertex/source data",
                self.other_vertex_and_source_data,
            ),
            (
                "Out-of-contract node transforms",
                self.out_of_contract_node_transforms,
            ),
            (
                "Animation targeting a matrix node",
                self.animation_targeting_matrix_nodes,
            ),
            (
                "Shared raw accessor payloads",
                self.shared_raw_accessor_payloads,
            ),
            (
                "Unreferenced accessor payloads",
                self.unreferenced_accessor_payloads,
            ),
            ("Image payload aliases", self.image_payload_aliases),
        ]
    }
}

/// A format-independent spelling of one FBX coordinate axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FbxCoordinateAxis {
    /// Positive X.
    PositiveX,
    /// Negative X.
    NegativeX,
    /// Positive Y.
    PositiveY,
    /// Negative Y.
    NegativeY,
    /// Positive Z.
    PositiveZ,
    /// Negative Z.
    NegativeZ,
    /// ufbx could not determine the axis.
    Unknown,
}

impl From<ufbx::CoordinateAxis> for FbxCoordinateAxis {
    fn from(value: ufbx::CoordinateAxis) -> Self {
        match value {
            ufbx::CoordinateAxis::PositiveX => Self::PositiveX,
            ufbx::CoordinateAxis::NegativeX => Self::NegativeX,
            ufbx::CoordinateAxis::PositiveY => Self::PositiveY,
            ufbx::CoordinateAxis::NegativeY => Self::NegativeY,
            ufbx::CoordinateAxis::PositiveZ => Self::PositiveZ,
            ufbx::CoordinateAxis::NegativeZ => Self::NegativeZ,
            ufbx::CoordinateAxis::Unknown => Self::Unknown,
        }
    }
}

/// Coordinate and unit normalization applied by the loader.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct FbxCoordinateNormalization {
    /// Original source up axis reported by ufbx.
    pub original_up_axis: FbxCoordinateAxis,
    /// Original source unit in metres reported by ufbx.
    pub original_unit_meters: f64,
    /// Target is right-handed, +Y up, and -Z forward.
    pub target_right_handed_y_up: bool,
    /// Target unit in metres.
    pub target_unit_meters: f64,
    /// ufbx adjusted transforms rather than preserving raw transform members.
    pub adjust_transforms: bool,
}

/// Stable source identity retained beside one normalized ufbx element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct FbxSourceIdentity {
    /// Stable index in the relevant ufbx typed list.
    pub source_index: usize,
    /// ufbx's typed id, which addresses that typed list.
    pub ufbx_typed_id: u32,
    /// ufbx's scene-wide element id, or zero for its generated root.
    ///
    /// This is deliberately not described as the raw FBX object id: ufbx
    /// assigns its own stable scene identity after parsing and normalization.
    pub ufbx_element_id: u32,
}

/// Provenance of inverse-bind matrices projected into the source sidecar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FbxBindMatrixProvenance {
    /// ufbx converted cluster bind matrices into target coordinates, then the
    /// loader derived `bind_to_world^-1 * geometry_to_world` per cluster.
    UfbxConvertedClusterMatrices,
}

/// Deterministic capability inventory captured from one successfully parsed FBX scene.
///
/// Every current Appendix D.4 row has a status, but those statuses deliberately
/// include unsupported and unverifiable states. Call
/// [`capability_facts`] to project those states into the format-neutral core
/// gate; #286-A never turns them into operation support.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct FbxScaleCapabilityInventory {
    /// Every Appendix D.4 domain, in named fields rather than an absence-based map.
    pub domains: FbxScaleDomainInventory,
    /// Coordinate and unit normalization applied before model construction.
    pub coordinate_normalization: FbxCoordinateNormalization,
    /// Every animation take is evaluated through `ufbx::bake_anim`.
    pub animation_takes_baked: bool,
    /// Authored FBX curve keys and interpolation are not retained.
    pub authored_curve_keys_preserved: bool,
    /// Number of source animation takes.
    pub animation_take_count: usize,
    /// Number of source animation curves discarded after baking.
    pub source_animation_curve_count: usize,
    /// Number of ufbx-generated geometry-transform helper nodes.
    pub generated_geometry_helper_node_count: usize,
    /// Number of ufbx-generated scale-compensation helper nodes.
    pub generated_scale_helper_node_count: usize,
    /// Whether the load boundary asks ufbx to compensate FBX inherit modes.
    pub inherit_modes_compensated: bool,
    /// Number of nodes whose original inherit mode or helper state required compensation.
    pub compensated_inherit_node_count: usize,
    /// Number of meshes for which ufbx generated missing normals.
    pub generated_normal_mesh_count: usize,
    /// Number of meshes still lacking normals after generation was requested.
    pub missing_normal_mesh_count: usize,
    /// Number of source skin deformers.
    pub skin_deformer_count: usize,
    /// Number of source skin clusters.
    pub skin_cluster_count: usize,
    /// Number of source skin deformers that declare no clusters or bind matrices.
    pub empty_skin_deformer_count: usize,
    /// Provenance of every available projected inverse-bind matrix.
    pub bind_matrix_provenance: FbxBindMatrixProvenance,
    /// Number of clusters missing a bone or a finite converted bind matrix.
    pub incomplete_bind_cluster_count: usize,
    /// Number of times multiple successfully projected clusters target one bone and overwrite its
    /// lossy convenience bind. Unreadable clusters are skipped, not counted as writes.
    pub bone_convenience_bind_overwrite_count: usize,
    /// Whether the loader invented identity matrices for missing bind evidence.
    pub identity_bind_defaults_invented: bool,
    /// Number of normalized vertices whose source influence list exceeded four entries.
    pub truncated_influence_vertex_count: usize,
    /// Number of source influences discarded by the four-slot limit.
    pub discarded_influence_count: usize,
    /// Number of normalized vertices whose retained weights changed during renormalization.
    pub renormalized_influence_vertex_count: usize,
    /// Number of non-finite, negative, or unrepresentable source influences rejected.
    pub rejected_influence_count: usize,
    /// Number of emitted skinned corners whose source vertex had no influence record.
    pub missing_skin_influence_corner_count: usize,
    /// Number of source faces that are not triangles.
    pub non_triangle_face_count: usize,
    /// Number of polygon faces with more than three corners that were triangulated.
    pub triangulated_face_count: usize,
    /// Number of point/line/empty faces omitted from triangle output.
    pub omitted_non_polygon_face_count: usize,
    /// Number of unindexed corners submitted to exact-bit welding.
    pub pre_weld_vertex_count: usize,
    /// Number of normalized vertices retained after exact-bit welding.
    pub post_weld_vertex_count: usize,
    /// Number of source meshes with more than one skin deformer.
    pub multiple_skin_deformer_mesh_count: usize,
    /// Number of dual-quaternion skin deformers not represented by the normalized model.
    pub dual_quaternion_skin_count: usize,
    /// Number of blend deformers (morph domains) not represented by the normalized model.
    pub blend_deformer_count: usize,
    /// Number of blend channels not represented by the normalized model.
    pub blend_channel_count: usize,
    /// Number of blend shapes not represented by the normalized model.
    pub blend_shape_count: usize,
    /// Number of geometry cache deformers not represented by the normalized model.
    pub cache_deformer_count: usize,
    /// Number of meshes carrying unsupported modeled-vertex payloads.
    pub unsupported_vertex_payload_mesh_count: usize,
    /// Number of cameras.
    pub camera_count: usize,
    /// Number of lights.
    pub light_count: usize,
    /// Number of shared mesh definitions with more than one node instance.
    pub shared_mesh_definition_count: usize,
    /// Number of source mesh definitions with no node instance and no normalized output mesh.
    pub uninstanced_mesh_definition_count: usize,
    /// Stable identities of the uninstanced source mesh definitions counted above.
    pub uninstanced_source_meshes: Vec<FbxSourceIdentity>,
    /// Number of user-defined source properties.
    pub user_defined_property_count: usize,
    /// Number of unknown or otherwise unmodeled source elements/scene records.
    pub unsupported_source_element_count: usize,
    /// Number of referenced external texture/video payloads.
    pub external_resource_count: usize,
    /// Node identities in stable ufbx source order.
    pub source_nodes: Vec<FbxSourceIdentity>,
    /// Mesh identities in stable ufbx source order.
    pub source_meshes: Vec<FbxSourceIdentity>,
    /// Skin-deformer identities in stable ufbx source order.
    pub source_skins: Vec<FbxSourceIdentity>,
}

/// One FBX document and the capability inventory captured from the same parse.
#[derive(Debug, Clone)]
pub struct FbxScaleSource {
    pub(crate) document: Document,
    pub(crate) inventory: FbxScaleCapabilityInventory,
}

impl FbxScaleSource {
    /// The normalized document carrying the documented ufbx source projection.
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// The conservative ufbx-side inventory.
    pub fn inventory(&self) -> &FbxScaleCapabilityInventory {
        &self.inventory
    }

    /// Consume the source wrapper and retain its normalized document.
    pub fn into_document(self) -> Document {
        self.document
    }
}

/// Project an FBX inventory into the format-neutral core capability gate.
///
/// `coverage` means every Appendix D.4 domain has an explicit status, not that
/// any domain is preserved losslessly. Support remains false: normalized transform stacks, baked
/// curves, rebuilt meshes, and unverifiable raw payload relationships are
/// recorded as unsupported facts rather than hidden behind absent flags.
pub fn capability_facts(inventory: &FbxScaleCapabilityInventory) -> ScaleCapabilityFacts {
    let mut facts = ScaleCapabilityFacts::default();
    facts.coverage = ScaleCapabilityCoverage::Complete;
    let morph_source_present = inventory.blend_deformer_count > 0
        || inventory.blend_channel_count > 0
        || inventory.blend_shape_count > 0;
    facts.morphs_present = morph_source_present;
    facts.morph_weights_present = morph_source_present;
    facts.cameras_present = inventory.camera_count > 0;
    facts.lights_present = inventory.light_count > 0;
    facts.instancing_present = inventory.shared_mesh_definition_count > 0;
    facts.unregistered_extensions_present = inventory.unsupported_source_element_count > 0;
    facts.extras_present = inventory.user_defined_property_count > 0;
    // FBX transform stacks and authored animation curves are normalized or
    // baked before Document construction, so their raw members are not in
    // the model even for the smallest accepted scene.
    facts.unknown_source_members_present = true;
    facts.non_triangle_primitives_present = inventory.non_triangle_face_count > 0;
    facts.unsupported_vertex_attributes_present = inventory.unsupported_vertex_payload_mesh_count
        > 0
        || inventory.uninstanced_mesh_definition_count > 0
        || inventory.multiple_skin_deformer_mesh_count > 0
        || inventory.dual_quaternion_skin_count > 0
        || inventory.cache_deformer_count > 0
        || inventory.missing_skin_influence_corner_count > 0
        || inventory.rejected_influence_count > 0
        || inventory.pre_weld_vertex_count != inventory.post_weld_vertex_count;
    facts.secondary_skin_influences_present = inventory.truncated_influence_vertex_count > 0;
    facts.inverse_bind_issues_present =
        inventory.incomplete_bind_cluster_count > 0 || inventory.empty_skin_deformer_count > 0;
    // ufbx exposes normalized objects, not accessor/image byte spans. A future
    // FBX writer must discharge this preservation obligation through the full
    // inventory route; #286-A cannot declare the source layout rewrite-safe.
    facts.unsafe_accessor_layout_present = true;
    facts.external_resources_present = inventory.external_resource_count > 0;
    facts
}

#[derive(Debug, Default)]
pub(crate) struct AssetConversionFacts {
    pub(crate) truncated_influence_vertex_count: usize,
    pub(crate) discarded_influence_count: usize,
    pub(crate) renormalized_influence_vertex_count: usize,
    pub(crate) rejected_influence_count: usize,
    pub(crate) missing_skin_influence_corner_count: usize,
    pub(crate) pre_weld_vertex_count: usize,
    pub(crate) post_weld_vertex_count: usize,
}

fn identity(index: usize, element: &ufbx::Element) -> FbxSourceIdentity {
    FbxSourceIdentity {
        source_index: index,
        ufbx_typed_id: element.typed_id,
        ufbx_element_id: element.element_id,
    }
}

pub(crate) fn inventory(
    scene: &ufbx::Scene,
    conversion: &AssetConversionFacts,
) -> FbxScaleCapabilityInventory {
    let non_triangle_face_count = scene
        .meshes
        .iter()
        .flat_map(|mesh| mesh.faces.iter())
        .filter(|face| face.num_indices != 3)
        .count();
    let triangulated_face_count = scene
        .meshes
        .iter()
        .flat_map(|mesh| mesh.faces.iter())
        .filter(|face| face.num_indices > 3)
        .count();
    let omitted_non_polygon_face_count = scene
        .meshes
        .iter()
        .flat_map(|mesh| mesh.faces.iter())
        .filter(|face| face.num_indices < 3)
        .count();
    let generated_normal_mesh_count = scene
        .meshes
        .iter()
        .filter(|mesh| mesh.generated_normals)
        .count();
    let missing_normal_mesh_count = scene
        .meshes
        .iter()
        .filter(|mesh| !mesh.vertex_normal.exists)
        .count();
    let skin_cluster_count = scene
        .skin_deformers
        .iter()
        .map(|skin| skin.clusters.len())
        .sum();
    let empty_skin_deformer_count = scene
        .skin_deformers
        .iter()
        .filter(|skin| skin.clusters.is_empty())
        .count();
    let incomplete_bind_cluster_count = scene
        .skin_clusters
        .iter()
        .filter(|cluster| super::project_cluster_bind(cluster).is_none())
        .count();
    let mut clusters_per_bone = std::collections::BTreeMap::<u32, usize>::new();
    for cluster in &scene.skin_clusters {
        if let (Some(node), Some(_)) = (&cluster.bone_node, super::project_cluster_bind(cluster)) {
            *clusters_per_bone.entry(node.element.typed_id).or_default() += 1;
        }
    }
    let bone_convenience_bind_overwrite_count = clusters_per_bone
        .values()
        .map(|count| count.saturating_sub(1))
        .sum();
    let multiple_skin_deformer_mesh_count = scene
        .meshes
        .iter()
        .filter(|mesh| mesh.skin_deformers.len() > 1)
        .count();
    let dual_quaternion_skin_count = scene
        .skin_deformers
        .iter()
        .filter(|skin| {
            skin.num_dq_weights > 0 || !matches!(skin.skinning_method, ufbx::SkinningMethod::Linear)
        })
        .count();
    let unsupported_vertex_payload_mesh_count = scene
        .meshes
        .iter()
        .filter(|mesh| mesh_has_unsupported_source_payload(mesh))
        .count();
    let shared_mesh_definition_count = scene
        .meshes
        .iter()
        .filter(|mesh| mesh.element.instances.len() > 1)
        .count();
    let uninstanced_source_meshes = scene
        .meshes
        .iter()
        .enumerate()
        .filter(|(_, mesh)| mesh.element.instances.is_empty())
        .map(|(index, mesh)| identity(index, &mesh.element))
        .collect::<Vec<_>>();
    let uninstanced_mesh_definition_count = uninstanced_source_meshes.len();
    let user_defined_property_count = scene
        .elements
        .iter()
        .flat_map(|element| element.props.props.iter())
        .filter(|prop| prop.flags.has_any(ufbx::PropFlags::USER_DEFINED))
        .count();
    let unsupported_source_element_count = unsupported_source_element_count(scene);
    let external_resource_count = scene
        .textures
        .iter()
        .filter(|texture| texture.content.is_empty() && texture.has_file)
        .count()
        + scene
            .videos
            .iter()
            .filter(|video| {
                video.content.is_empty()
                    && (!video.filename.is_empty()
                        || !video.relative_filename.is_empty()
                        || !video.absolute_filename.is_empty())
            })
            .count();
    let compensated_inherit_node_count = scene
        .nodes
        .iter()
        .filter(|node| {
            node.original_inherit_mode != node.inherit_mode
                || node.is_scale_helper
                || node.is_scale_compensate_parent
        })
        .count();

    let animation = if scene.anim_stacks.is_empty() {
        FbxScaleDomainStatus::Absent
    } else {
        FbxScaleDomainStatus::Baked
    };
    let domains = FbxScaleDomainInventory {
        rest_hierarchy: FbxScaleDomainStatus::Normalized,
        translation_animation: animation,
        rotation_and_scale_animation: animation,
        root_motion_and_velocity: if scene.anim_stacks.is_empty() {
            FbxScaleDomainStatus::Absent
        } else {
            FbxScaleDomainStatus::Derived
        },
        base_mesh_geometry: if scene.meshes.is_empty() {
            FbxScaleDomainStatus::Absent
        } else if uninstanced_mesh_definition_count > 0 {
            FbxScaleDomainStatus::Unsupported
        } else {
            FbxScaleDomainStatus::Rebuilt
        },
        morphs: if scene.blend_deformers.is_empty()
            && scene.blend_channels.is_empty()
            && scene.blend_shapes.is_empty()
        {
            FbxScaleDomainStatus::Absent
        } else {
            FbxScaleDomainStatus::Unsupported
        },
        skin_binds: if scene.skin_deformers.is_empty() {
            FbxScaleDomainStatus::Absent
        } else if incomplete_bind_cluster_count > 0 || empty_skin_deformer_count > 0 {
            FbxScaleDomainStatus::Unsupported
        } else {
            FbxScaleDomainStatus::Derived
        },
        cameras_and_lights: if scene.cameras.is_empty() && scene.lights.is_empty() {
            FbxScaleDomainStatus::Absent
        } else {
            FbxScaleDomainStatus::Unsupported
        },
        collision_and_custom_data: if unsupported_source_element_count == 0
            && user_defined_property_count == 0
        {
            FbxScaleDomainStatus::Absent
        } else {
            FbxScaleDomainStatus::Unsupported
        },
        other_vertex_and_source_data: if unsupported_vertex_payload_mesh_count > 0
            || uninstanced_mesh_definition_count > 0
            || multiple_skin_deformer_mesh_count > 0
            || dual_quaternion_skin_count > 0
            || conversion.truncated_influence_vertex_count > 0
            || conversion.missing_skin_influence_corner_count > 0
            || conversion.rejected_influence_count > 0
            || !scene.blend_deformers.is_empty()
            || !scene.blend_channels.is_empty()
            || !scene.blend_shapes.is_empty()
            || !scene.cache_deformers.is_empty()
            || !scene.cache_files.is_empty()
        {
            FbxScaleDomainStatus::Unsupported
        } else if !scene.meshes.is_empty() {
            FbxScaleDomainStatus::Rebuilt
        } else {
            FbxScaleDomainStatus::Absent
        },
        out_of_contract_node_transforms: FbxScaleDomainStatus::Normalized,
        animation_targeting_matrix_nodes: animation,
        shared_raw_accessor_payloads: FbxScaleDomainStatus::Unverifiable,
        unreferenced_accessor_payloads: FbxScaleDomainStatus::Unverifiable,
        image_payload_aliases: FbxScaleDomainStatus::Unverifiable,
    };

    FbxScaleCapabilityInventory {
        domains,
        coordinate_normalization: FbxCoordinateNormalization {
            original_up_axis: scene.settings.original_axis_up.into(),
            original_unit_meters: scene.settings.original_unit_meters,
            target_right_handed_y_up: true,
            target_unit_meters: 1.0,
            adjust_transforms: matches!(
                scene.metadata.space_conversion,
                ufbx::SpaceConversion::AdjustTransforms
            ),
        },
        animation_takes_baked: true,
        authored_curve_keys_preserved: false,
        animation_take_count: scene.anim_stacks.len(),
        source_animation_curve_count: scene.anim_curves.len(),
        generated_geometry_helper_node_count: scene
            .nodes
            .iter()
            .filter(|node| node.is_geometry_transform_helper)
            .count(),
        generated_scale_helper_node_count: scene
            .nodes
            .iter()
            .filter(|node| node.is_scale_helper)
            .count(),
        inherit_modes_compensated: matches!(
            scene.metadata.inherit_mode_handling,
            ufbx::InheritModeHandling::Compensate
        ),
        compensated_inherit_node_count,
        generated_normal_mesh_count,
        missing_normal_mesh_count,
        skin_deformer_count: scene.skin_deformers.len(),
        skin_cluster_count,
        empty_skin_deformer_count,
        bind_matrix_provenance: FbxBindMatrixProvenance::UfbxConvertedClusterMatrices,
        incomplete_bind_cluster_count,
        bone_convenience_bind_overwrite_count,
        identity_bind_defaults_invented: false,
        truncated_influence_vertex_count: conversion.truncated_influence_vertex_count,
        discarded_influence_count: conversion.discarded_influence_count,
        renormalized_influence_vertex_count: conversion.renormalized_influence_vertex_count,
        rejected_influence_count: conversion.rejected_influence_count,
        missing_skin_influence_corner_count: conversion.missing_skin_influence_corner_count,
        non_triangle_face_count,
        triangulated_face_count,
        omitted_non_polygon_face_count,
        pre_weld_vertex_count: conversion.pre_weld_vertex_count,
        post_weld_vertex_count: conversion.post_weld_vertex_count,
        multiple_skin_deformer_mesh_count,
        dual_quaternion_skin_count,
        blend_deformer_count: scene.blend_deformers.len(),
        blend_channel_count: scene.blend_channels.len(),
        blend_shape_count: scene.blend_shapes.len(),
        cache_deformer_count: scene.cache_deformers.len(),
        unsupported_vertex_payload_mesh_count,
        camera_count: scene.cameras.len(),
        light_count: scene.lights.len(),
        shared_mesh_definition_count,
        uninstanced_mesh_definition_count,
        uninstanced_source_meshes,
        user_defined_property_count,
        unsupported_source_element_count,
        external_resource_count,
        source_nodes: scene
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| identity(index, &node.element))
            .collect(),
        source_meshes: scene
            .meshes
            .iter()
            .enumerate()
            .map(|(index, mesh)| identity(index, &mesh.element))
            .collect(),
        source_skins: scene
            .skin_deformers
            .iter()
            .enumerate()
            .map(|(index, skin)| identity(index, &skin.element))
            .collect(),
    }
}

/// Classify every field in `ufbx::Mesh` at one structural boundary. Omitting
/// `..` is deliberate: a ufbx upgrade that adds mesh payload must fail to
/// compile until extraction either models it or this predicate refuses it.
fn mesh_has_unsupported_source_payload(mesh: &ufbx::Mesh) -> bool {
    let ufbx::Mesh {
        element: _,
        num_vertices: _,
        num_indices: _,
        num_faces: _,
        num_triangles: _,
        num_edges: _,
        max_face_triangles: _,
        num_empty_faces: _,
        num_point_faces: _,
        num_line_faces: _,
        faces: _,
        // Authored face/edge members are not retained by triangle extraction.
        face_smoothing,
        face_material: _,
        face_group,
        face_hole,
        edges,
        edge_smoothing,
        edge_crease,
        edge_visibility,
        vertex_indices: _,
        vertices: _,
        vertex_first_index: _,
        vertex_position: _,
        vertex_normal: _,
        vertex_uv: _,
        vertex_tangent,
        vertex_bitangent,
        vertex_color,
        vertex_crease,
        uv_sets,
        color_sets,
        materials: _,
        face_groups,
        // Mesh parts and skinned views are parser-derived indexes/results.
        material_parts: _,
        face_group_parts: _,
        material_part_usage_order: _,
        skinned_is_local: _,
        skinned_position: _,
        skinned_normal: _,
        // Deformer kinds have dedicated inventory counters.
        skin_deformers: _,
        blend_deformers: _,
        cache_deformers: _,
        all_deformers: _,
        subdivision_preview_levels,
        subdivision_render_levels,
        subdivision_display_mode,
        subdivision_boundary,
        subdivision_uv_boundary,
        // Winding conversion and generated-normal state are consumed/counted.
        reversed_winding: _,
        generated_normals: _,
        subdivision_evaluated,
        subdivision_result,
        from_tessellated_nurbs,
    } = mesh;

    !face_smoothing.is_empty()
        || !face_group.is_empty()
        || !face_hole.is_empty()
        || !edges.is_empty()
        || !edge_smoothing.is_empty()
        || !edge_crease.is_empty()
        || !edge_visibility.is_empty()
        || vertex_tangent.exists
        || vertex_bitangent.exists
        || vertex_color.exists
        || vertex_crease.exists
        || uv_sets.len() > 1
        || !color_sets.is_empty()
        || !face_groups.is_empty()
        || *subdivision_preview_levels > 0
        || *subdivision_render_levels > 0
        || !matches!(
            subdivision_display_mode,
            ufbx::SubdivisionDisplayMode::Disabled
        )
        || !matches!(subdivision_boundary, ufbx::SubdivisionBoundary::Default)
        || !matches!(subdivision_uv_boundary, ufbx::SubdivisionBoundary::Default)
        || *subdivision_evaluated
        || subdivision_result.is_some()
        || *from_tessellated_nurbs
}

/// Classify every field in `ufbx::Scene` at one exhaustive structural
/// boundary. Omitting `..` is deliberate: a ufbx upgrade that adds a typed
/// list must fail to compile until that list receives a classification.
fn unsupported_source_element_count(scene: &ufbx::Scene) -> usize {
    let ufbx::Scene {
        metadata: _,
        settings: _,
        root_node: _,
        anim: _,
        // Unknown and source kinds without a normalized core representation.
        unknowns,
        nodes: _,
        meshes: _,
        // Cameras and lights have dedicated counts/core facts.
        lights: _,
        cameras: _,
        // Bone/empty attributes normalize into the complete node projection.
        bones: _,
        empties: _,
        line_curves,
        nurbs_curves,
        nurbs_surfaces,
        nurbs_trim_surfaces,
        nurbs_trim_boundaries,
        procedural_geometries,
        stereo_cameras,
        camera_switchers,
        markers,
        lod_groups,
        // Skin rows have dedicated bind/influence counts and sidecars.
        skin_deformers: _,
        skin_clusters: _,
        // Deformers are counted separately; their subordinate payload rows
        // are still unmodeled source elements.
        blend_deformers: _,
        blend_channels,
        blend_shapes,
        cache_deformers: _,
        cache_files,
        // The loader rebuilds its documented material/texture subset;
        // external payload absence is counted separately.
        materials: _,
        textures: _,
        videos: _,
        shaders,
        shader_bindings,
        // Animation lists are evaluated through bake_anim.
        anim_stacks: _,
        anim_layers: _,
        anim_values: _,
        anim_curves: _,
        display_layers,
        selection_sets,
        selection_nodes,
        characters,
        constraints,
        audio_layers,
        audio_clips,
        poses,
        metadata_objects,
        // Texture-file records carry source linkage beyond the rebuilt
        // texture subset, so they are conservatively unmodeled.
        texture_files,
        // Scene-wide structural indexes are parser-derived views, not source
        // element domains that need independent semantic counting.
        elements: _,
        connections_src: _,
        connections_dst: _,
        elements_by_name: _,
        dom_root: _,
    } = scene;

    unknowns.len()
        + line_curves.len()
        + nurbs_curves.len()
        + nurbs_surfaces.len()
        + nurbs_trim_surfaces.len()
        + nurbs_trim_boundaries.len()
        + procedural_geometries.len()
        + stereo_cameras.len()
        + camera_switchers.len()
        + markers.len()
        + lod_groups.len()
        + blend_channels.len()
        + blend_shapes.len()
        + cache_files.len()
        + shaders.len()
        + shader_bindings.len()
        + display_layers.len()
        + selection_sets.len()
        + selection_nodes.len()
        + characters.len()
        + constraints.len()
        + audio_layers.len()
        + audio_clips.len()
        + poses.len()
        + metadata_objects.len()
        + texture_files.len()
}
