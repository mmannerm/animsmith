//! Conservative ufbx-side scale capability inventory.

use animsmith_core::scale::{ScaleCapabilityCoverage, ScaleCapabilityFacts};
use animsmith_core::{
    Document, LoadedSource, SourceConstructKindV1, SourceFactsViewV1, SourceResourceLocatorV1,
    SourceSetCoverageStateV1,
};
use serde::Serialize;

/// How one Appendix D.4 domain reaches the normalized FBX document.
///
/// These values describe semantic ingestion only. None claims raw FBX byte,
/// object-property, curve-key, or payload-span preservation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
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

    /// Semantic domains whose normalized representation must be complete
    /// before the narrow FBX rest/bind bridge may stage a GLB.
    ///
    /// The three raw-span rows are deliberately excluded: the bridge never
    /// rewrites FBX bytes, so it serializes a private GLB and proves that
    /// GLB's raw spans instead. Keeping this as typed fields rather than
    /// display labels makes a newly added semantic domain fail closed until
    /// this policy is deliberately updated.
    fn rest_bind_semantic_statuses(&self) -> [FbxScaleDomainStatus; 12] {
        [
            self.rest_hierarchy,
            self.translation_animation,
            self.rotation_and_scale_animation,
            self.root_motion_and_velocity,
            self.base_mesh_geometry,
            self.morphs,
            self.skin_binds,
            self.cameras_and_lights,
            self.collision_and_custom_data,
            self.other_vertex_and_source_data,
            self.out_of_contract_node_transforms,
            self.animation_targeting_matrix_nodes,
        ]
    }

    /// The raw-span rows whose FBX status is expected to be unverifiable.
    fn rest_bind_raw_span_statuses(&self) -> [FbxScaleDomainStatus; 3] {
        [
            self.shared_raw_accessor_payloads,
            self.unreferenced_accessor_payloads,
            self.image_payload_aliases,
        ]
    }
}

/// A format-independent spelling of one FBX coordinate axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
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
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FbxCoordinateNormalization {
    /// Advisory `OriginalUpAxis` value reported by ufbx.
    ///
    /// This is not the effective `UpAxis`/`FrontAxis`/`CoordAxis` basis.
    pub original_up_axis: FbxCoordinateAxis,
    /// Advisory `OriginalUnitScaleFactor` value in metres reported by ufbx.
    ///
    /// This is not the effective `UnitScaleFactor` source unit.
    pub original_unit_meters: f64,
    /// Target is right-handed, +Y up, and -Z forward.
    pub target_right_handed_y_up: bool,
    /// Target unit in metres.
    pub target_unit_meters: f64,
    /// ufbx adjusted transforms rather than preserving raw transform members.
    pub adjust_transforms: bool,
}

/// Stable source identity retained beside one normalized ufbx element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
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
/// gate; #286-A never turns them into operation support. This is also the
/// frozen source projection serialized by scale-evidence v5: a new inventory
/// fact requires a new evidence version rather than silently changing v5.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
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
    /// Number of point/line faces omitted from triangle output.
    pub omitted_non_polygon_face_count: usize,
    /// Number of source mesh definitions that declare no faces.
    pub empty_mesh_definition_count: usize,
    /// Stable identities of the zero-face source mesh definitions counted above.
    pub empty_source_meshes: Vec<FbxSourceIdentity>,
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

/// One immutable FBX source/document owner and scale capability inventory
/// captured from the same parse.
#[derive(Debug)]
pub struct FbxScaleSource {
    pub(crate) source: LoadedSource,
    pub(crate) inventory: FbxScaleCapabilityInventory,
}

impl FbxScaleSource {
    /// The normalized document carrying the documented ufbx source projection.
    pub fn document(&self) -> &Document {
        self.source.document()
    }

    /// The bounded importer-sensitive facts retained from the same ufbx parse.
    pub fn source_facts(&self) -> SourceFactsViewV1<'_> {
        self.source.source_facts()
    }

    /// The conservative ufbx-side inventory.
    pub fn inventory(&self) -> &FbxScaleCapabilityInventory {
        &self.inventory
    }

    /// Consume the source wrapper and retain its normalized document.
    pub fn into_document(self) -> Document {
        self.source.into_document()
    }

    pub(crate) fn into_source(self) -> LoadedSource {
        self.source
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
        || inventory.empty_mesh_definition_count > 0
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

/// Project scale capabilities from one immutable captured FBX source.
///
/// The operation inventory keeps its detailed normalization/bake ledger. The
/// shared raw-source facts independently supply custom/unknown construct and
/// resource presence, and partial shared coverage always fails closed.
pub fn capability_facts_for_source(source: &FbxScaleSource) -> ScaleCapabilityFacts {
    join_source_facts(source, capability_facts(source.inventory()))
}

fn join_source_facts(
    source: &FbxScaleSource,
    mut facts: ScaleCapabilityFacts,
) -> ScaleCapabilityFacts {
    let source_facts = source.source_facts();
    if [
        source_facts.constructs().coverage().state(),
        source_facts.resources().coverage().state(),
    ]
    .into_iter()
    .any(|state| state != SourceSetCoverageStateV1::Complete)
    {
        facts.coverage = ScaleCapabilityCoverage::Unavailable;
    }
    for row in source_facts.constructs().rows() {
        match row.kind() {
            SourceConstructKindV1::CustomProperty => facts.extras_present = true,
            SourceConstructKindV1::UnknownElement => {
                facts.unknown_source_members_present = true;
            }
            SourceConstructKindV1::Extension => facts.unregistered_extensions_present = true,
        }
    }
    if source_facts.resources().rows().iter().any(|row| {
        !matches!(
            row.locator(),
            SourceResourceLocatorV1::Embedded | SourceResourceLocatorV1::DataUri
        )
    }) {
        facts.external_resources_present = true;
    }
    facts
}

/// Project the narrow FBX subset that can enter rest/bind scaling.
///
/// The accepted operation rewrites a freshly serialized GLB rather than the
/// FBX container, so the three raw-span rows are intentionally
/// [`FbxScaleDomainStatus::Unverifiable`]. Every semantic domain must still
/// be complete: an unsupported row, incomplete bind evidence, altered skin
/// influences, external resources, or an incomplete coordinate projection is
/// a stable refusal before the producer can stage any output.
pub fn rest_bind_capability_facts(
    inventory: &FbxScaleCapabilityInventory,
) -> Result<ScaleCapabilityFacts, String> {
    if inventory
        .domains
        .rest_bind_semantic_statuses()
        .into_iter()
        .any(|status| {
            matches!(
                status,
                FbxScaleDomainStatus::Unsupported | FbxScaleDomainStatus::Unverifiable
            )
        })
    {
        return Err("FBX capability inventory leaves a semantic domain incomplete".into());
    }
    if inventory
        .domains
        .rest_bind_raw_span_statuses()
        .into_iter()
        .any(|status| !matches!(status, FbxScaleDomainStatus::Unverifiable))
    {
        return Err("FBX capability inventory has an unexpected raw-span projection".into());
    }
    if !inventory.coordinate_normalization.target_right_handed_y_up
        || inventory.coordinate_normalization.target_unit_meters != 1.0
        || !inventory.coordinate_normalization.adjust_transforms
        || !inventory.animation_takes_baked
        || inventory.authored_curve_keys_preserved
        || !inventory.inherit_modes_compensated
        || inventory.incomplete_bind_cluster_count != 0
        || inventory.empty_skin_deformer_count != 0
        || inventory.missing_normal_mesh_count != 0
        || inventory.bone_convenience_bind_overwrite_count != 0
        || inventory.identity_bind_defaults_invented
        || inventory.truncated_influence_vertex_count != 0
        || inventory.discarded_influence_count != 0
        || inventory.renormalized_influence_vertex_count != 0
        || inventory.rejected_influence_count != 0
        || inventory.missing_skin_influence_corner_count != 0
        || inventory.non_triangle_face_count != 0
        || inventory.triangulated_face_count != 0
        || inventory.omitted_non_polygon_face_count != 0
        || inventory.external_resource_count != 0
    {
        return Err("FBX capability inventory cannot prove the selected rest/bind domain".into());
    }
    // Reuse the complete conservative projection for every counter and
    // source-domain fact. The two flags it sets solely because this is not a
    // raw FBX rewriter are discharged by the private GLB staging/proof
    // boundary below; every other fact continues to gate the operation.
    let mut facts = capability_facts(inventory);
    facts.unknown_source_members_present = false;
    facts.unsafe_accessor_layout_present = false;
    if !facts.is_supported_for(
        animsmith_core::scale::ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 1.0,
        },
    ) {
        return Err("FBX capability inventory contains an unsupported rest/bind domain".into());
    }
    Ok(facts)
}

/// Project the narrow FBX rest/bind subset from one captured source.
///
/// Shared construct/resource coverage is checked before the older
/// operation-specific inventory. This prevents a truncated positive-only raw
/// projection from being treated as proof of absence.
///
/// # Errors
///
/// Returns a stable refusal when shared raw facts or the scale inventory
/// cannot prove the selected domain.
pub fn rest_bind_capability_facts_for_source(
    source: &FbxScaleSource,
) -> Result<ScaleCapabilityFacts, String> {
    let mut shared = ScaleCapabilityFacts::default();
    shared.coverage = ScaleCapabilityCoverage::Complete;
    shared = join_source_facts(source, shared);
    if shared.coverage != ScaleCapabilityCoverage::Complete
        || shared.extras_present
        || shared.unknown_source_members_present
        || shared.unregistered_extensions_present
        || shared.external_resources_present
    {
        return Err("FBX raw-source facts cannot prove the selected rest/bind domain".into());
    }
    let facts = join_source_facts(source, rest_bind_capability_facts(source.inventory())?);
    if facts.is_supported_for(
        animsmith_core::scale::ScaleOperation::RestBindUniformScale {
            source_skin_index: 0,
            source_root_node_index: 0,
            expected_factor: 1.0,
        },
    ) {
        Ok(facts)
    } else {
        Err("FBX raw-source facts contain an unsupported rest/bind domain".into())
    }
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
    construct_counts: crate::source_facts::SourceConstructCounts,
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
    let empty_source_meshes = scene
        .meshes
        .iter()
        .enumerate()
        .filter(|(_, mesh)| mesh.faces.is_empty())
        .map(|(index, mesh)| identity(index, &mesh.element))
        .collect::<Vec<_>>();
    let empty_mesh_definition_count = empty_source_meshes.len();
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
    let user_defined_property_count = construct_counts.user_defined_property_count;
    let unsupported_source_element_count = construct_counts.unsupported_source_element_count;
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

    let stackless_animation_present = scene.anim_stacks.is_empty()
        && (!scene.anim_layers.is_empty()
            || !scene.anim_values.is_empty()
            || !scene.anim_curves.is_empty());
    let animation = if !scene.anim_stacks.is_empty() {
        FbxScaleDomainStatus::Baked
    } else if stackless_animation_present {
        // No take was available to bake, but authored curve/value/layer rows
        // were parsed and discarded by normalized clip extraction.
        FbxScaleDomainStatus::Unsupported
    } else {
        FbxScaleDomainStatus::Absent
    };
    let domains = FbxScaleDomainInventory {
        rest_hierarchy: FbxScaleDomainStatus::Normalized,
        translation_animation: animation,
        rotation_and_scale_animation: animation,
        root_motion_and_velocity: match animation {
            FbxScaleDomainStatus::Baked => FbxScaleDomainStatus::Derived,
            status => status,
        },
        base_mesh_geometry: if scene.meshes.is_empty() {
            FbxScaleDomainStatus::Absent
        } else if uninstanced_mesh_definition_count > 0
            || omitted_non_polygon_face_count > 0
            || empty_mesh_definition_count > 0
        {
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
            || omitted_non_polygon_face_count > 0
            || empty_mesh_definition_count > 0
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
        empty_mesh_definition_count,
        empty_source_meshes,
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

#[cfg(test)]
mod tests {
    use super::*;
    use animsmith_core::{
        InputIdentity, RawSourceFactsBuilderV1, SourceConstructFactV1, SourceFactDomainV1,
        SourceFormatV1, SourceLoaderDispositionV1, SourceLogicalLocatorV1, SourceProvenanceV1,
        SourceResourceKindV1, SourceResourceReferenceV1, SourceTextV1,
    };
    use std::path::PathBuf;

    fn captured_with(configure: impl FnOnce(&mut RawSourceFactsBuilderV1)) -> FbxScaleSource {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata/rigged_triangle.fbx");
        let baseline = crate::load_scale_source(&fixture).expect("checked-in FBX fixture loads");
        let document = baseline.document().clone();
        let inventory = baseline.inventory().clone();
        let identity: InputIdentity = baseline.source_facts().primary_identity().clone();
        let mut builder = RawSourceFactsBuilderV1::new(SourceFormatV1::Fbx, identity);
        configure(&mut builder);
        let source = builder.finish(document).expect("synthetic raw facts bind");
        FbxScaleSource { source, inventory }
    }

    fn parser_provenance(path: &str) -> SourceProvenanceV1 {
        SourceProvenanceV1::parser_projected(
            SourceLogicalLocatorV1::fbx_parser_path(path).expect("test parser path is valid"),
        )
    }

    #[test]
    fn source_aware_rest_bind_rejects_partial_relevant_coverage() {
        for source in [
            captured_with(|builder| {
                builder.mark_budget_exceeded(SourceFactDomainV1::Constructs);
                builder.mark_complete(SourceFactDomainV1::Resources);
            }),
            captured_with(|builder| {
                builder.mark_complete(SourceFactDomainV1::Constructs);
                builder.mark_budget_exceeded(SourceFactDomainV1::Resources);
            }),
        ] {
            assert!(
                rest_bind_capability_facts_for_source(&source).is_err(),
                "partial construct/resource coverage must fail before the operation inventory"
            );
        }
    }

    #[test]
    fn source_aware_rest_bind_rejects_positive_shared_domains() {
        for kind in [
            SourceConstructKindV1::CustomProperty,
            SourceConstructKindV1::UnknownElement,
        ] {
            let source = captured_with(|builder| {
                builder.push_construct(
                    SourceConstructFactV1::new(
                        0,
                        kind,
                        SourceTextV1::new("synthetic").expect("bounded test name"),
                        false,
                        1,
                        SourceLoaderDispositionV1::Unsupported,
                        parser_provenance("fbx:synthetic/construct"),
                    )
                    .expect("positive test construct"),
                );
                builder.mark_complete(SourceFactDomainV1::Constructs);
                builder.mark_complete(SourceFactDomainV1::Resources);
            });
            assert!(rest_bind_capability_facts_for_source(&source).is_err());
        }

        let external = captured_with(|builder| {
            builder.mark_complete(SourceFactDomainV1::Constructs);
            builder.push_resource(SourceResourceReferenceV1::new(
                0,
                SourceResourceKindV1::Texture,
                0,
                SourceResourceLocatorV1::classify("texture.png"),
                SourceLoaderDispositionV1::Unknown,
                parser_provenance("fbx:textures/0/filename"),
            ));
            builder.mark_complete(SourceFactDomainV1::Resources);
        });
        assert!(rest_bind_capability_facts_for_source(&external).is_err());
    }
}
