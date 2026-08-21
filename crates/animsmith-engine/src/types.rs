use animsmith_core::{InputIdentity, SourceFormatV1};
use std::collections::BTreeMap;

/// Exact four-field key selecting one revisioned engine profile.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProfileSelection {
    family: String,
    profile_revision: u32,
    engine_version: String,
    importer: String,
}

impl ProfileSelection {
    /// Construct an exact profile selection without aliases or version ranges.
    pub fn new(
        family: impl Into<String>,
        profile_revision: u32,
        engine_version: impl Into<String>,
        importer: impl Into<String>,
    ) -> Self {
        Self {
            family: family.into(),
            profile_revision,
            engine_version: engine_version.into(),
            importer: importer.into(),
        }
    }

    /// Stable profile family id.
    pub fn family(&self) -> &str {
        &self.family
    }

    /// Exact profile revision.
    pub const fn profile_revision(&self) -> u32 {
        self.profile_revision
    }

    /// Exact target engine version.
    pub fn engine_version(&self) -> &str {
        &self.engine_version
    }

    /// Exact importer id.
    pub fn importer(&self) -> &str {
        &self.importer
    }
}

/// Stable id for one immutable fact in every V1 profile record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FactId {
    /// Bounded AnimSmith input containers accepted by the profile.
    AcceptedInputs,
    /// How imported animation assets are addressed.
    AnimationAddressability,
    /// Target coordinate basis.
    TargetCoordinateBasis,
    /// Target linear unit.
    TargetLinearUnit,
    /// Importer's source-to-target unit-conversion control.
    UnitConversionControl,
    /// Importer's source-to-target axis-conversion control.
    AxisConversionControl,
    /// Exact axis-conversion transform.
    ExactAxisConversion,
    /// Resulting imported hierarchy scale.
    ResultingHierarchyScale,
    /// Whether a whole end frame is required at clip boundaries.
    WholeEndFrameRequired,
    /// Import handling of animation channels.
    AnimationChannelHandling,
    /// Import handling of source extensions.
    ExtensionHandling,
    /// Import handling of source constructs.
    ConstructHandling,
    /// How imported animation targets are addressed.
    AnimationTargetAddressability,
    /// How root-motion sources are addressed.
    RootMotionAddressability,
}

impl FactId {
    /// Stable digest/source-reference spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AcceptedInputs => "accepted_inputs",
            Self::AnimationAddressability => "animation_addressability",
            Self::TargetCoordinateBasis => "target_coordinate_basis",
            Self::TargetLinearUnit => "target_linear_unit",
            Self::UnitConversionControl => "unit_conversion_control",
            Self::AxisConversionControl => "axis_conversion_control",
            Self::ExactAxisConversion => "exact_axis_conversion",
            Self::ResultingHierarchyScale => "resulting_hierarchy_scale",
            Self::WholeEndFrameRequired => "whole_end_frame_required",
            Self::AnimationChannelHandling => "animation_channel_handling",
            Self::ExtensionHandling => "extension_handling",
            Self::ConstructHandling => "construct_handling",
            Self::AnimationTargetAddressability => "animation_target_addressability",
            Self::RootMotionAddressability => "root_motion_addressability",
        }
    }
}

/// Coordinate-system handedness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Handedness {
    /// Left-handed coordinates.
    Left,
    /// Right-handed coordinates.
    Right,
}

/// Positive world axis used as up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UpAxis {
    /// Positive X is up.
    X,
    /// Positive Y is up.
    Y,
    /// Positive Z is up.
    Z,
}

/// Known target coordinate basis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CoordinateBasis {
    /// Coordinate-system handedness.
    pub handedness: Handedness,
    /// Positive world up axis.
    pub up_axis: UpAxis,
    /// Signed target forward axis.
    pub forward_axis: ForwardAxis,
}

/// Signed world axis used as forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ForwardAxis {
    /// Positive X is forward.
    PositiveX,
    /// Negative X is forward.
    NegativeX,
    /// Positive Y is forward.
    PositiveY,
    /// Negative Y is forward.
    NegativeY,
    /// Positive Z is forward.
    PositiveZ,
    /// Negative Z is forward.
    NegativeZ,
}

/// Known target linear unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LinearUnit {
    /// Metre.
    Metre,
    /// Centimetre.
    Centimetre,
}

/// Stable setting id in the closed V1 vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SettingId {
    /// Unity document-level unit-conversion toggle.
    ConvertUnits,
    /// Unity document-level axis-baking toggle.
    BakeAxisConversion,
    /// Unity Generic document-level exact source-transform path.
    RootMotionSource,
    /// Unity per-clip root-rotation policy.
    RootRotation,
    /// Unity per-clip vertical root-position policy.
    RootPositionY,
    /// Unity per-clip horizontal root-position policy.
    RootPositionXz,
}

impl SettingId {
    /// Stable public configuration spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConvertUnits => "convert_units",
            Self::BakeAxisConversion => "bake_axis_conversion",
            Self::RootMotionSource => "root_motion_source",
            Self::RootRotation => "root_rotation",
            Self::RootPositionY => "root_position_y",
            Self::RootPositionXz => "root_position_xz",
        }
    }

    pub(crate) fn from_str(value: &str) -> Option<Self> {
        match value {
            "convert_units" => Some(Self::ConvertUnits),
            "bake_axis_conversion" => Some(Self::BakeAxisConversion),
            "root_motion_source" => Some(Self::RootMotionSource),
            "root_rotation" => Some(Self::RootRotation),
            "root_position_y" => Some(Self::RootPositionY),
            "root_position_xz" => Some(Self::RootPositionXz),
            _ => None,
        }
    }
}

/// A known importer control relevant to source-to-target conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConversionControl {
    /// Behavior is controlled by one declared profile setting.
    ProfileSetting(SettingId),
    /// Behavior is exposed by the importer but is not a V1 profile setting.
    ImporterOption,
}

/// Known importer treatment for a fact domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImportHandling {
    /// The importer retains the domain.
    Preserved,
    /// The importer converts the domain.
    Converted,
    /// The importer discards the domain.
    Discarded,
    /// The importer does not support the domain.
    Unsupported,
}

/// Known animation-target addressability behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TargetAddressability {
    /// Targets are addressed by a stable id derived from their name path.
    NamePathDerivedId,
}

/// Known animation-asset addressability behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AnimationAddressability {
    /// Bevy addresses each glTF animation by its source-array index through
    /// `GltfAssetLabel::Animation(index)`.
    GltfAssetLabel,
}

/// Known root-motion addressability behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RootMotionAddressability {
    /// A bounded exact source-transform path selects the motion node.
    ExactSourceTransformPath,
    /// Humanoid Avatar/body semantics determine root motion.
    HumanoidAvatarBody,
}

/// Typed value of a known immutable profile fact.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FactValue {
    /// Exact accepted AnimSmith input formats.
    AcceptedFormats(Vec<SourceFormatV1>),
    /// Animation-asset addressability.
    AnimationAddressability(AnimationAddressability),
    /// Target coordinate basis.
    CoordinateBasis(CoordinateBasis),
    /// Target linear unit.
    LinearUnit(LinearUnit),
    /// Source-to-target conversion control.
    ConversionControl(ConversionControl),
    /// Boolean predicate.
    Boolean(bool),
    /// Import handling of a domain.
    ImportHandling(ImportHandling),
    /// Animation-target addressability.
    TargetAddressability(TargetAddressability),
    /// Root-motion addressability.
    RootMotionAddressability(RootMotionAddressability),
}

/// Evidence state of one profile fact.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FactState {
    /// Supported known value.
    Known(FactValue),
    /// Primary evidence does not establish a value.
    Unknown,
    /// The fact domain genuinely does not apply.
    NotApplicable,
}

/// One stable fact and its explicit evidence state.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProfileFact {
    id: FactId,
    state: FactState,
}

impl ProfileFact {
    pub(crate) fn new(id: FactId, state: FactState) -> Self {
        Self { id, state }
    }

    /// Stable fact id.
    pub const fn id(&self) -> FactId {
        self.id
    }

    /// Explicit known, unknown, or not-applicable state.
    pub const fn state(&self) -> &FactState {
        &self.state
    }
}

/// Configuration scope of a setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SettingScope {
    /// One value governs the imported document and all clips.
    Document,
    /// One materialized value is required for each real clip.
    Clip,
}

/// Closed value domain of a setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SettingDomain {
    /// Boolean value.
    Boolean,
    /// `bake` or `extract`.
    BakeOrExtract,
    /// Bounded exact source-transform path.
    SourceTransformPath,
}

/// Whether a descriptor applies to a profile revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SettingApplicability {
    /// The setting applies.
    Applicable,
    /// The setting genuinely does not apply.
    NotApplicable,
}

/// Verified default status of a setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DefaultStatus {
    /// The caller must declare a value because no default is verified.
    RequiredWithoutDefault,
    /// Default behavior is irrelevant because the setting does not apply.
    NotApplicable,
}

/// Immutable descriptor for one stable setting id.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SettingDescriptor {
    id: SettingId,
    scope: SettingScope,
    domain: SettingDomain,
    applicability: SettingApplicability,
    default_status: DefaultStatus,
}

impl SettingDescriptor {
    pub(crate) const fn new(
        id: SettingId,
        scope: SettingScope,
        domain: SettingDomain,
        applicability: SettingApplicability,
        default_status: DefaultStatus,
    ) -> Self {
        Self {
            id,
            scope,
            domain,
            applicability,
            default_status,
        }
    }

    /// Stable setting id.
    pub const fn id(&self) -> SettingId {
        self.id
    }

    /// Required declaration scope.
    pub const fn scope(&self) -> SettingScope {
        self.scope
    }

    /// Closed value domain.
    pub const fn domain(&self) -> SettingDomain {
        self.domain
    }

    /// Applicability to this exact profile revision.
    pub const fn applicability(&self) -> SettingApplicability {
        self.applicability
    }

    /// Verified default status.
    pub const fn default_status(&self) -> DefaultStatus {
        self.default_status
    }
}

/// Exact policy for a Unity root transform component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BakeOrExtract {
    /// Bake the component into the pose.
    Bake,
    /// Extract the component as root motion.
    Extract,
}

/// Closed public value vocabulary for engine settings.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SettingValue {
    /// Boolean setting value.
    Boolean(bool),
    /// Root-component bake/extract policy.
    BakeOrExtract(BakeOrExtract),
    /// Exact source-transform path, validated during static resolution.
    SourceTransformPath(String),
}

/// String-keyed setting declarations used at the TOML-free public boundary.
pub type SettingMap = BTreeMap<String, SettingValue>;

/// Full public input to phase-one static resolution.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EngineDeclaration {
    /// Exact profile selection, or none for engine-neutral behavior.
    pub selection: Option<ProfileSelection>,
    /// Declared document settings. `Some` retains an explicitly empty table.
    pub document_settings: Option<SettingMap>,
    /// Selector-keyed per-clip setting declarations.
    pub clip_settings: BTreeMap<String, SettingMap>,
}

/// One primary source retained by an immutable profile record.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PrimarySource {
    id: &'static str,
    target_version: &'static str,
    url: &'static str,
    verified_on: &'static str,
    supported_facts: Vec<FactId>,
    supported_settings: Vec<SettingId>,
}

impl PrimarySource {
    pub(crate) fn new(
        id: &'static str,
        target_version: &'static str,
        url: &'static str,
        verified_on: &'static str,
        mut supported_facts: Vec<FactId>,
        mut supported_settings: Vec<SettingId>,
    ) -> Self {
        supported_facts.sort_by_key(|id| id.as_str());
        supported_settings.sort_by_key(|id| id.as_str());
        Self {
            id,
            target_version,
            url,
            verified_on,
            supported_facts,
            supported_settings,
        }
    }

    /// Stable source id.
    pub const fn id(&self) -> &'static str {
        self.id
    }

    /// Version of the source's target product/specification.
    pub const fn target_version(&self) -> &'static str {
        self.target_version
    }

    /// Primary-source URL.
    pub const fn url(&self) -> &'static str {
        self.url
    }

    /// ISO date on which this source was verified.
    pub const fn verified_on(&self) -> &'static str {
        self.verified_on
    }

    /// Stable fact ids supported by the source.
    pub fn supported_facts(&self) -> &[FactId] {
        &self.supported_facts
    }

    /// Stable setting ids supported by the source.
    pub fn supported_settings(&self) -> &[SettingId] {
        &self.supported_settings
    }

    #[cfg(test)]
    pub(crate) fn reverse_supported_ids_for_test(&mut self) {
        self.supported_facts.reverse();
        self.supported_settings.reverse();
    }
}

/// One immutable, revisioned registry profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineProfile {
    pub(crate) selection: ProfileSelection,
    pub(crate) fact_bundle_urn: &'static str,
    pub(crate) facts: Vec<ProfileFact>,
    pub(crate) settings: Vec<SettingDescriptor>,
    pub(crate) sources: Vec<PrimarySource>,
    pub(crate) facts_identity: InputIdentity,
}

impl EngineProfile {
    /// Full exact selection tuple retained by this profile.
    pub const fn selection(&self) -> &ProfileSelection {
        &self.selection
    }

    /// Revisioned fact-bundle URN.
    pub const fn fact_bundle_urn(&self) -> &'static str {
        self.fact_bundle_urn
    }

    /// Bounded AnimSmith V1 input formats accepted by the profile.
    pub fn accepted_inputs(&self) -> &[SourceFormatV1] {
        match self.fact(FactId::AcceptedInputs).map(ProfileFact::state) {
            Some(FactState::Known(FactValue::AcceptedFormats(formats))) => formats,
            _ => &[],
        }
    }

    /// Complete typed fact inventory in stable-id order.
    pub fn facts(&self) -> &[ProfileFact] {
        &self.facts
    }

    /// Complete descriptor inventory in stable-id order.
    pub fn setting_descriptors(&self) -> &[SettingDescriptor] {
        &self.settings
    }

    /// Primary sources in stable-id order.
    pub fn sources(&self) -> &[PrimarySource] {
        &self.sources
    }

    /// SHA-256 plus byte count of the canonical immutable record encoding.
    pub const fn facts_identity(&self) -> &InputIdentity {
        &self.facts_identity
    }

    /// Look up one fact by stable id.
    pub fn fact(&self, id: FactId) -> Option<&ProfileFact> {
        self.facts.iter().find(|fact| fact.id == id)
    }

    /// Look up one setting descriptor, including not-applicable descriptors.
    pub fn setting_descriptor(&self, id: SettingId) -> Option<&SettingDescriptor> {
        self.settings.iter().find(|descriptor| descriptor.id == id)
    }
}
