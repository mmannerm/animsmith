//! Registry-independent engine profile facts and resolved settings.
//!
//! These wire values deliberately mirror the closed V1 vocabulary owned by
//! `animsmith-engine` without making core depend on that crate. Their
//! canonical encoders preserve the byte preimages introduced with the V1
//! engine registry.

use crate::{InputIdentity, SourceFormatV1};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use std::collections::BTreeSet;

/// Semantic contract for a self-contained V1 engine profile record.
pub const ENGINE_PROFILE_FACTS_V1_ID: &str = "urn:animsmith:engine-profile-facts:1";
/// Semantic contract for fully materialized V1 engine settings.
pub const RESOLVED_ENGINE_SETTINGS_V1_ID: &str = "urn:animsmith:resolved-engine-settings:1";
/// Maximum rows in any individual V1 profile or resolved-settings collection.
pub const ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS: usize = 4_096;
/// Maximum aggregate profile and materialized-setting rows retained by one lint file.
pub const ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS: usize = 65_536;
/// Maximum UTF-8 bytes in one retained profile/settings string.
pub const ENGINE_CONTRACT_V1_MAX_TEXT_BYTES: usize = 4_096;
/// Maximum aggregate UTF-8 bytes retained by V1 provenance and predictions in one lint file.
pub const ENGINE_CONTRACT_V1_MAX_TOTAL_TEXT_BYTES: usize = 8 * 1024 * 1024;

const ENGINE_FACTS_PREIMAGE_DOMAIN: &str = "animsmith-engine-facts-v1";
const ENGINE_SETTINGS_PREIMAGE_DOMAIN: &str = "animsmith-engine-settings-v1";

/// Fixed-field-order token encoder shared by V1 prediction identities.
///
/// Each token is prefixed by its unsigned eight-byte big-endian UTF-8 byte
/// length. Counts are decimal UTF-8 tokens.
#[derive(Debug, Default)]
pub(crate) struct CanonicalEncoder(Vec<u8>);

impl CanonicalEncoder {
    /// Start a composite with its domain token.
    pub(crate) fn new(domain: &str) -> Self {
        let mut encoder = Self::default();
        encoder.token(domain);
        encoder
    }

    /// Append one UTF-8 token.
    pub(crate) fn token(&mut self, token: impl AsRef<str>) {
        let bytes = token.as_ref().as_bytes();
        self.0
            .extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        self.0.extend_from_slice(bytes);
    }

    /// Append a field-name token.
    pub(crate) fn field(&mut self, field: &'static str) {
        self.token(field);
    }

    /// Append a collection count as a minimal decimal token.
    pub(crate) fn count(&mut self, count: usize) {
        self.token(count.to_string());
    }

    /// Hash the complete canonical preimage.
    pub(crate) fn identity(self) -> InputIdentity {
        InputIdentity::from_bytes(&self.0)
    }

    /// Return the complete canonical preimage bytes.
    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

/// Append the canonical four-token representation of an input identity.
pub(crate) fn encode_input_identity(encoder: &mut CanonicalEncoder, identity: &InputIdentity) {
    encoder.token("sha256");
    encoder.token(identity.sha256());
    encoder.token("bytes");
    encoder.token(identity.bytes().to_string());
}

impl Serialize for SourceFormatV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(source_format_name(*self))
    }
}

impl<'de> Deserialize<'de> for SourceFormatV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.as_str() {
            "gltf_json" => Ok(Self::GltfJson),
            "glb" => Ok(Self::Glb),
            "fbx" => Ok(Self::Fbx),
            other => Err(D::Error::custom(format!(
                "unknown V1 source format {other:?}"
            ))),
        }
    }
}

impl<'de> Deserialize<'de> for InputIdentity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct WireIdentity {
            sha256: String,
            bytes: u64,
        }

        let wire = WireIdentity::deserialize(deserializer)?;
        if wire.sha256.len() != 64
            || !wire
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(D::Error::custom(
                "input identity sha256 must be exactly 64 lowercase hexadecimal digits",
            ));
        }
        let mut digest = [0_u8; 32];
        for (index, pair) in wire.sha256.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            digest[index] = (hex_nibble(pair[0]).expect("validated hexadecimal") << 4)
                | hex_nibble(pair[1]).expect("validated hexadecimal");
        }
        Ok(InputIdentity::from_sha256_digest(digest, wire.bytes))
    }
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

/// Exact four-field key selecting one revisioned engine profile.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineProfileSelectionV1 {
    family: String,
    profile_revision: u32,
    engine_version: String,
    importer: String,
}

impl EngineProfileSelectionV1 {
    /// Construct an exact profile selection.
    ///
    /// # Errors
    ///
    /// Returns [`EngineContractError`] when a retained string is empty or
    /// exceeds the V1 per-string limit.
    pub fn new(
        family: impl Into<String>,
        profile_revision: u32,
        engine_version: impl Into<String>,
        importer: impl Into<String>,
    ) -> Result<Self, EngineContractError> {
        let selection = Self {
            family: family.into(),
            profile_revision,
            engine_version: engine_version.into(),
            importer: importer.into(),
        };
        selection.validate()?;
        Ok(selection)
    }

    /// Stable engine-family id.
    pub fn family(&self) -> &str {
        &self.family
    }

    /// Exact immutable profile revision.
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

    fn validate(&self) -> Result<(), EngineContractError> {
        validate_required_text("selection.family", &self.family)?;
        validate_required_text("selection.engine_version", &self.engine_version)?;
        validate_required_text("selection.importer", &self.importer)
    }

    fn retained_text_bytes(&self) -> Result<usize, EngineContractError> {
        checked_sum(
            "profile retained text",
            [
                self.family.len(),
                self.engine_version.len(),
                self.importer.len(),
            ],
        )
    }
}

/// Stable id for one immutable V1 profile fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineFactIdV1 {
    /// Bounded input formats accepted by the profile.
    AcceptedInputs,
    /// How imported animation assets are addressed.
    AnimationAddressability,
    /// Target coordinate basis.
    TargetCoordinateBasis,
    /// Target linear unit.
    TargetLinearUnit,
    /// Source-to-target unit-conversion control.
    UnitConversionControl,
    /// Source-to-target axis-conversion control.
    AxisConversionControl,
    /// Exact axis-conversion transform.
    ExactAxisConversion,
    /// Resulting imported hierarchy scale.
    ResultingHierarchyScale,
    /// Whether clip boundaries require a whole end frame.
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

impl EngineFactIdV1 {
    /// Stable wire and canonical spelling.
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

const ALL_FACT_IDS: [EngineFactIdV1; 14] = [
    EngineFactIdV1::AcceptedInputs,
    EngineFactIdV1::AnimationAddressability,
    EngineFactIdV1::AnimationChannelHandling,
    EngineFactIdV1::AnimationTargetAddressability,
    EngineFactIdV1::AxisConversionControl,
    EngineFactIdV1::ConstructHandling,
    EngineFactIdV1::ExactAxisConversion,
    EngineFactIdV1::ExtensionHandling,
    EngineFactIdV1::ResultingHierarchyScale,
    EngineFactIdV1::RootMotionAddressability,
    EngineFactIdV1::TargetCoordinateBasis,
    EngineFactIdV1::TargetLinearUnit,
    EngineFactIdV1::UnitConversionControl,
    EngineFactIdV1::WholeEndFrameRequired,
];

/// Coordinate-system handedness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineHandednessV1 {
    /// Left-handed coordinates.
    Left,
    /// Right-handed coordinates.
    Right,
}

/// Positive world axis used as up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineUpAxisV1 {
    /// Positive X is up.
    X,
    /// Positive Y is up.
    Y,
    /// Positive Z is up.
    Z,
}

/// Signed world axis used as forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineForwardAxisV1 {
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

/// Known target coordinate basis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineCoordinateBasisV1 {
    /// Coordinate-system handedness.
    pub handedness: EngineHandednessV1,
    /// Positive world up axis.
    pub up_axis: EngineUpAxisV1,
    /// Signed target forward axis.
    pub forward_axis: EngineForwardAxisV1,
}

/// Known target linear unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineLinearUnitV1 {
    /// Metre.
    Metre,
    /// Centimetre.
    Centimetre,
}

/// Stable id in the closed V1 engine-setting vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineSettingIdV1 {
    /// Unity document-level unit-conversion toggle.
    ConvertUnits,
    /// Unity document-level axis-baking toggle.
    BakeAxisConversion,
    /// Unity Generic exact source-transform path.
    RootMotionSource,
    /// Unity per-clip root-rotation policy.
    RootRotation,
    /// Unity per-clip vertical root-position policy.
    RootPositionY,
    /// Unity per-clip horizontal root-position policy.
    RootPositionXz,
}

impl EngineSettingIdV1 {
    /// Stable wire and canonical spelling.
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
}

/// Known importer control relevant to source-to-target conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineConversionControlV1 {
    /// Behavior is controlled by one declared profile setting.
    ProfileSetting(EngineSettingIdV1),
    /// Behavior is exposed by the importer but is not a V1 profile setting.
    ImporterOption,
}

/// Known importer treatment for a fact domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineImportHandlingV1 {
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineTargetAddressabilityV1 {
    /// Targets use a stable id derived from their name path.
    NamePathDerivedId,
}

/// Known animation-asset addressability behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineAnimationAddressabilityV1 {
    /// Animations have typed glTF asset labels by name or index.
    GltfAssetLabel,
}

/// Known root-motion addressability behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineRootMotionAddressabilityV1 {
    /// A bounded exact source-transform path selects the motion node.
    ExactSourceTransformPath,
    /// Humanoid Avatar/body semantics determine root motion.
    HumanoidAvatarBody,
}

/// Typed value of one known immutable profile fact.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineFactValueV1 {
    /// Exact accepted input formats.
    AcceptedFormats(Vec<SourceFormatV1>),
    /// Animation-asset addressability.
    AnimationAddressability(EngineAnimationAddressabilityV1),
    /// Target coordinate basis.
    CoordinateBasis(EngineCoordinateBasisV1),
    /// Target linear unit.
    LinearUnit(EngineLinearUnitV1),
    /// Source-to-target conversion control.
    ConversionControl(EngineConversionControlV1),
    /// Boolean predicate.
    Boolean(bool),
    /// Import handling of a domain.
    ImportHandling(EngineImportHandlingV1),
    /// Animation-target addressability.
    TargetAddressability(EngineTargetAddressabilityV1),
    /// Root-motion addressability.
    RootMotionAddressability(EngineRootMotionAddressabilityV1),
}

/// Evidence state of one immutable profile fact.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineFactStateV1 {
    /// Supported known value.
    Known(EngineFactValueV1),
    /// Primary evidence does not establish a value.
    Unknown,
    /// The fact domain genuinely does not apply.
    NotApplicable,
}

/// One stable fact and its explicit evidence state.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineProfileFactV1 {
    id: EngineFactIdV1,
    state: EngineFactStateV1,
}

impl EngineProfileFactV1 {
    /// Construct one profile fact.
    pub const fn new(id: EngineFactIdV1, state: EngineFactStateV1) -> Self {
        Self { id, state }
    }

    /// Stable fact id.
    pub const fn id(&self) -> EngineFactIdV1 {
        self.id
    }

    /// Explicit evidence state.
    pub const fn state(&self) -> &EngineFactStateV1 {
        &self.state
    }
}

/// Configuration scope of a setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineSettingScopeV1 {
    /// One value governs the imported document and all clips.
    Document,
    /// One materialized value is required for each actual clip.
    Clip,
}

/// Closed value domain of a setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineSettingDomainV1 {
    /// Boolean value.
    Boolean,
    /// `bake` or `extract`.
    BakeOrExtract,
    /// Bounded exact source-transform path.
    SourceTransformPath,
}

/// Whether a descriptor applies to a profile revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineSettingApplicabilityV1 {
    /// The setting applies.
    Applicable,
    /// The setting genuinely does not apply.
    NotApplicable,
}

/// Verified default status of a setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineDefaultStatusV1 {
    /// The caller must declare a value because no default is verified.
    RequiredWithoutDefault,
    /// Default behavior is irrelevant because the setting does not apply.
    NotApplicable,
}

/// Immutable descriptor for one stable setting id.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineSettingDescriptorV1 {
    id: EngineSettingIdV1,
    scope: EngineSettingScopeV1,
    domain: EngineSettingDomainV1,
    applicability: EngineSettingApplicabilityV1,
    default_status: EngineDefaultStatusV1,
}

impl EngineSettingDescriptorV1 {
    /// Construct one immutable setting descriptor.
    pub const fn new(
        id: EngineSettingIdV1,
        scope: EngineSettingScopeV1,
        domain: EngineSettingDomainV1,
        applicability: EngineSettingApplicabilityV1,
        default_status: EngineDefaultStatusV1,
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
    pub const fn id(&self) -> EngineSettingIdV1 {
        self.id
    }

    /// Required setting scope.
    pub const fn scope(&self) -> EngineSettingScopeV1 {
        self.scope
    }

    /// Closed value domain.
    pub const fn domain(&self) -> EngineSettingDomainV1 {
        self.domain
    }

    /// Applicability to this exact profile revision.
    pub const fn applicability(&self) -> EngineSettingApplicabilityV1 {
        self.applicability
    }

    /// Verified default status.
    pub const fn default_status(&self) -> EngineDefaultStatusV1 {
        self.default_status
    }
}

/// One primary source retained by an immutable profile record.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnginePrimarySourceV1 {
    id: String,
    target_version: String,
    url: String,
    verified_on: String,
    supported_fact_ids: Vec<EngineFactIdV1>,
    supported_setting_ids: Vec<EngineSettingIdV1>,
}

impl EnginePrimarySourceV1 {
    /// Construct one primary-source row, canonicalizing its supported-id sets.
    ///
    /// # Errors
    ///
    /// Returns [`EngineContractError`] for empty or oversized text, oversized
    /// id sets, or duplicate supported ids.
    pub fn new(
        id: impl Into<String>,
        target_version: impl Into<String>,
        url: impl Into<String>,
        verified_on: impl Into<String>,
        mut supported_fact_ids: Vec<EngineFactIdV1>,
        mut supported_setting_ids: Vec<EngineSettingIdV1>,
    ) -> Result<Self, EngineContractError> {
        supported_fact_ids.sort_by_key(|id| id.as_str());
        supported_setting_ids.sort_by_key(|id| id.as_str());
        let source = Self {
            id: id.into(),
            target_version: target_version.into(),
            url: url.into(),
            verified_on: verified_on.into(),
            supported_fact_ids,
            supported_setting_ids,
        };
        source.validate(true)?;
        Ok(source)
    }

    /// Stable primary-source id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Source target version.
    pub fn target_version(&self) -> &str {
        &self.target_version
    }

    /// Primary-source URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// ISO verification date.
    pub fn verified_on(&self) -> &str {
        &self.verified_on
    }

    /// Stable fact ids supported by this source.
    pub fn supported_fact_ids(&self) -> &[EngineFactIdV1] {
        &self.supported_fact_ids
    }

    /// Stable setting ids supported by this source.
    pub fn supported_setting_ids(&self) -> &[EngineSettingIdV1] {
        &self.supported_setting_ids
    }

    fn validate(&self, require_order: bool) -> Result<(), EngineContractError> {
        validate_required_text("primary_sources.id", &self.id)?;
        validate_required_text("primary_sources.target_version", &self.target_version)?;
        validate_required_text("primary_sources.url", &self.url)?;
        validate_required_text("primary_sources.verified_on", &self.verified_on)?;
        validate_collection_len(
            "primary_sources.supported_fact_ids",
            self.supported_fact_ids.len(),
        )?;
        validate_collection_len(
            "primary_sources.supported_setting_ids",
            self.supported_setting_ids.len(),
        )?;
        validate_unique_order(
            "primary_sources.supported_fact_ids",
            &self.supported_fact_ids,
            |id| id.as_str(),
            require_order,
        )?;
        validate_unique_order(
            "primary_sources.supported_setting_ids",
            &self.supported_setting_ids,
            |id| id.as_str(),
            require_order,
        )
    }

    fn retained_text_bytes(&self) -> Result<usize, EngineContractError> {
        checked_sum(
            "profile retained text",
            [
                self.id.len(),
                self.target_version.len(),
                self.url.len(),
                self.verified_on.len(),
            ],
        )
    }
}

/// Registry-independent, self-contained immutable engine profile record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedEngineProfileV1 {
    schema: String,
    selection: EngineProfileSelectionV1,
    fact_bundle_urn: String,
    identity: InputIdentity,
    facts: Vec<EngineProfileFactV1>,
    setting_descriptors: Vec<EngineSettingDescriptorV1>,
    primary_sources: Vec<EnginePrimarySourceV1>,
}

impl ResolvedEngineProfileV1 {
    /// Construct and canonically order one self-contained profile record.
    ///
    /// # Errors
    ///
    /// Returns [`EngineContractError`] when the profile is incomplete,
    /// internally inconsistent, duplicated, or exceeds a V1 bound.
    pub fn new(
        selection: EngineProfileSelectionV1,
        fact_bundle_urn: impl Into<String>,
        mut facts: Vec<EngineProfileFactV1>,
        mut setting_descriptors: Vec<EngineSettingDescriptorV1>,
        mut primary_sources: Vec<EnginePrimarySourceV1>,
    ) -> Result<Self, EngineContractError> {
        for fact in &mut facts {
            if let EngineFactStateV1::Known(EngineFactValueV1::AcceptedFormats(formats)) =
                &mut fact.state
            {
                formats.sort_by_key(|format| source_format_name(*format));
            }
        }
        facts.sort_by_key(|fact| fact.id.as_str());
        setting_descriptors.sort_by_key(|descriptor| descriptor.id.as_str());
        primary_sources.sort_by(|left, right| left.id.cmp(&right.id));
        let mut profile = Self {
            schema: ENGINE_PROFILE_FACTS_V1_ID.to_owned(),
            selection,
            fact_bundle_urn: fact_bundle_urn.into(),
            identity: InputIdentity::from_bytes(&[]),
            facts,
            setting_descriptors,
            primary_sources,
        };
        profile.validate_semantics(true, false)?;
        profile.identity = profile.computed_identity();
        Ok(profile)
    }

    /// Contract id carried in the `schema` field.
    pub fn contract_id(&self) -> &str {
        &self.schema
    }

    /// Exact four-field profile selection.
    pub const fn selection(&self) -> &EngineProfileSelectionV1 {
        &self.selection
    }

    /// Selected immutable profile fact-bundle URN.
    pub fn fact_bundle_urn(&self) -> &str {
        &self.fact_bundle_urn
    }

    /// SHA-256 plus byte count of the unchanged #464 facts preimage.
    pub const fn facts_identity(&self) -> &InputIdentity {
        &self.identity
    }

    /// Complete typed fact inventory in stable-id order.
    pub fn facts(&self) -> &[EngineProfileFactV1] {
        &self.facts
    }

    /// Complete descriptor inventory in stable-id order.
    pub fn setting_descriptors(&self) -> &[EngineSettingDescriptorV1] {
        &self.setting_descriptors
    }

    /// Primary-source rows in stable-id order.
    pub fn primary_sources(&self) -> &[EnginePrimarySourceV1] {
        &self.primary_sources
    }

    /// Look up one fact by stable id.
    pub fn fact(&self, id: EngineFactIdV1) -> Option<&EngineProfileFactV1> {
        self.facts.iter().find(|fact| fact.id == id)
    }

    /// Whether the exact embedded `accepted_inputs` fact accepts `format`.
    pub fn accepts_format(&self, format: SourceFormatV1) -> bool {
        matches!(
            self.fact(EngineFactIdV1::AcceptedInputs)
                .map(EngineProfileFactV1::state),
            Some(EngineFactStateV1::Known(EngineFactValueV1::AcceptedFormats(formats)))
                if formats.contains(&format)
        )
    }

    /// Look up one setting descriptor, including not-applicable descriptors.
    pub fn setting_descriptor(&self, id: EngineSettingIdV1) -> Option<&EngineSettingDescriptorV1> {
        self.setting_descriptors
            .iter()
            .find(|descriptor| descriptor.id == id)
    }

    /// Look up one primary source by stable id.
    pub fn source(&self, id: &str) -> Option<&EnginePrimarySourceV1> {
        self.primary_sources.iter().find(|source| source.id == id)
    }

    /// Revalidate all profile semantics and its identity.
    ///
    /// # Errors
    ///
    /// Returns [`EngineContractError`] for any invalid wire or cross-reference.
    pub fn validate(&self) -> Result<(), EngineContractError> {
        self.validate_semantics(true, true)
    }

    /// Append the complete unchanged #464 facts preimage, including its domain.
    pub(crate) fn encode_preimage(&self, encoder: &mut CanonicalEncoder) {
        encoder.token(ENGINE_FACTS_PREIMAGE_DOMAIN);
        encode_profile_key(encoder, &self.selection);
        encoder.field("fact_bundle_urn");
        encoder.token(&self.fact_bundle_urn);
        encoder.field("facts");
        encoder.count(self.facts.len());
        for fact in &self.facts {
            encoder.token(fact.id.as_str());
            encode_fact_state(encoder, &fact.state);
        }
        encoder.field("setting_descriptors");
        encoder.count(self.setting_descriptors.len());
        for descriptor in &self.setting_descriptors {
            encoder.token(descriptor.id.as_str());
            encoder.token(setting_scope_name(descriptor.scope));
            encoder.token(setting_domain_name(descriptor.domain));
            encoder.token(match descriptor.applicability {
                EngineSettingApplicabilityV1::Applicable => "applicable",
                EngineSettingApplicabilityV1::NotApplicable => "not_applicable",
            });
            encoder.token(match descriptor.default_status {
                EngineDefaultStatusV1::RequiredWithoutDefault => "required_without_default",
                EngineDefaultStatusV1::NotApplicable => "not_applicable",
            });
        }
        encoder.field("sources");
        encoder.count(self.primary_sources.len());
        for source in &self.primary_sources {
            encoder.token(&source.id);
            encoder.token(&source.target_version);
            encoder.token(&source.url);
            encoder.token(&source.verified_on);
            encoder.count(source.supported_fact_ids.len());
            for id in &source.supported_fact_ids {
                encoder.token(id.as_str());
            }
            encoder.count(source.supported_setting_ids.len());
            for id in &source.supported_setting_ids {
                encoder.token(id.as_str());
            }
        }
    }

    pub(crate) fn retained_rows(&self) -> Result<usize, EngineContractError> {
        let nested = self.primary_sources.iter().map(|source| {
            source
                .supported_fact_ids
                .len()
                .checked_add(source.supported_setting_ids.len())
                .ok_or(EngineContractError::ArithmeticOverflow {
                    field: "profile retained rows",
                })
        });
        checked_sum_results(
            "profile retained rows",
            [
                self.facts.len(),
                self.setting_descriptors.len(),
                self.primary_sources.len(),
            ],
            nested,
        )
    }

    pub(crate) fn retained_text_bytes(&self) -> Result<usize, EngineContractError> {
        let base = self
            .selection
            .retained_text_bytes()?
            .checked_add(self.fact_bundle_urn.len())
            .ok_or(EngineContractError::ArithmeticOverflow {
                field: "profile retained text",
            })?;
        checked_sum_results(
            "profile retained text",
            [base],
            self.primary_sources
                .iter()
                .map(EnginePrimarySourceV1::retained_text_bytes),
        )
    }

    fn computed_identity(&self) -> InputIdentity {
        let mut encoder = CanonicalEncoder::default();
        self.encode_preimage(&mut encoder);
        encoder.identity()
    }

    fn validate_semantics(
        &self,
        require_order: bool,
        verify_identity: bool,
    ) -> Result<(), EngineContractError> {
        validate_schema("profile.schema", &self.schema, ENGINE_PROFILE_FACTS_V1_ID)?;
        self.selection.validate()?;
        validate_required_text("profile.fact_bundle_urn", &self.fact_bundle_urn)?;
        validate_collection_len("profile.facts", self.facts.len())?;
        validate_collection_len(
            "profile.setting_descriptors",
            self.setting_descriptors.len(),
        )?;
        validate_collection_len("profile.primary_sources", self.primary_sources.len())?;
        validate_unique_order(
            "profile.facts",
            &self.facts,
            |fact| fact.id.as_str(),
            require_order,
        )?;
        if self.facts.len() != ALL_FACT_IDS.len()
            || !self
                .facts
                .iter()
                .zip(ALL_FACT_IDS)
                .all(|(fact, expected)| fact.id == expected)
        {
            return Err(EngineContractError::InvalidFactInventory);
        }
        for fact in &self.facts {
            validate_fact_value(fact)?;
            if let EngineFactStateV1::Known(EngineFactValueV1::ConversionControl(
                EngineConversionControlV1::ProfileSetting(setting),
            )) = &fact.state
                && self.setting_descriptor(*setting).is_none()
            {
                return Err(EngineContractError::InvalidFactValue { fact: fact.id });
            }
        }
        if !matches!(
            self.fact(EngineFactIdV1::AcceptedInputs)
                .map(EngineProfileFactV1::state),
            Some(EngineFactStateV1::Known(
                EngineFactValueV1::AcceptedFormats(formats)
            )) if !formats.is_empty()
        ) {
            return Err(EngineContractError::InvalidAcceptedInputs);
        }
        validate_unique_order(
            "profile.setting_descriptors",
            &self.setting_descriptors,
            |descriptor| descriptor.id.as_str(),
            require_order,
        )?;
        for descriptor in &self.setting_descriptors {
            if !matches!(
                (descriptor.applicability, descriptor.default_status),
                (
                    EngineSettingApplicabilityV1::Applicable,
                    EngineDefaultStatusV1::RequiredWithoutDefault
                ) | (
                    EngineSettingApplicabilityV1::NotApplicable,
                    EngineDefaultStatusV1::NotApplicable
                )
            ) {
                return Err(EngineContractError::InvalidDescriptorDefault {
                    setting: descriptor.id,
                });
            }
        }
        validate_unique_order(
            "profile.primary_sources",
            &self.primary_sources,
            |source| source.id.as_str(),
            require_order,
        )?;
        for source in &self.primary_sources {
            source.validate(require_order)?;
            for fact in &source.supported_fact_ids {
                let Some(row) = self.fact(*fact) else {
                    return Err(EngineContractError::UnknownSourceFact {
                        source_id: source.id.clone(),
                        fact: *fact,
                    });
                };
                if !matches!(row.state, EngineFactStateV1::Known(_)) {
                    return Err(EngineContractError::SourceReferencesNonKnownFact {
                        source_id: source.id.clone(),
                        fact: *fact,
                    });
                }
            }
            for setting in &source.supported_setting_ids {
                if self.setting_descriptor(*setting).is_none() {
                    return Err(EngineContractError::UnknownSourceSetting {
                        source_id: source.id.clone(),
                        setting: *setting,
                    });
                }
            }
        }
        for fact in &self.facts {
            if matches!(fact.state, EngineFactStateV1::Known(_))
                && !self
                    .primary_sources
                    .iter()
                    .any(|source| source.supported_fact_ids.contains(&fact.id))
            {
                return Err(EngineContractError::UnreferencedKnownFact { fact: fact.id });
            }
        }
        for descriptor in &self.setting_descriptors {
            if !self
                .primary_sources
                .iter()
                .any(|source| source.supported_setting_ids.contains(&descriptor.id))
            {
                return Err(EngineContractError::UnreferencedSetting {
                    setting: descriptor.id,
                });
            }
        }
        let rows = self.retained_rows()?;
        if rows > ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS {
            return Err(EngineContractError::TooManyAggregateRows {
                found: rows,
                max: ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS,
            });
        }
        let text = self.retained_text_bytes()?;
        if text > ENGINE_CONTRACT_V1_MAX_TOTAL_TEXT_BYTES {
            return Err(EngineContractError::TooMuchAggregateText {
                found: text,
                max: ENGINE_CONTRACT_V1_MAX_TOTAL_TEXT_BYTES,
            });
        }
        if verify_identity && self.identity != self.computed_identity() {
            return Err(EngineContractError::IdentityMismatch {
                contract: ENGINE_PROFILE_FACTS_V1_ID,
            });
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolvedEngineProfileWireV1 {
    schema: String,
    selection: EngineProfileSelectionV1,
    fact_bundle_urn: String,
    identity: InputIdentity,
    facts: Vec<EngineProfileFactV1>,
    setting_descriptors: Vec<EngineSettingDescriptorV1>,
    primary_sources: Vec<EnginePrimarySourceV1>,
}

#[derive(Debug)]
pub(crate) enum EngineContractDecodeError {
    Shape(serde_json::Error),
    Semantic(EngineContractError),
}

impl ResolvedEngineProfileV1 {
    fn from_wire(wire: ResolvedEngineProfileWireV1) -> Result<Self, EngineContractError> {
        let profile = Self {
            schema: wire.schema,
            selection: wire.selection,
            fact_bundle_urn: wire.fact_bundle_urn,
            identity: wire.identity,
            facts: wire.facts,
            setting_descriptors: wire.setting_descriptors,
            primary_sources: wire.primary_sources,
        };
        profile.validate()?;
        Ok(profile)
    }
}

pub(crate) fn decode_resolved_engine_profile_v1(
    raw: &str,
) -> Result<ResolvedEngineProfileV1, EngineContractDecodeError> {
    let wire = serde_json::from_str(raw).map_err(EngineContractDecodeError::Shape)?;
    ResolvedEngineProfileV1::from_wire(wire).map_err(EngineContractDecodeError::Semantic)
}

impl<'de> Deserialize<'de> for ResolvedEngineProfileV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::from_wire(ResolvedEngineProfileWireV1::deserialize(deserializer)?)
            .map_err(D::Error::custom)
    }
}

/// Exact policy for a root-transform component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineBakeOrExtractV1 {
    /// Bake the component into the pose.
    Bake,
    /// Extract the component as root motion.
    Extract,
}

/// Closed public value vocabulary for fully materialized engine settings.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineSettingValueV1 {
    /// Boolean setting value.
    Boolean(bool),
    /// Root-component bake/extract policy.
    BakeOrExtract(EngineBakeOrExtractV1),
    /// Bounded exact source-transform path.
    SourceTransformPath(String),
}

/// One stable-id-keyed materialized setting value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineSettingRowV1 {
    id: EngineSettingIdV1,
    value: EngineSettingValueV1,
}

impl EngineSettingRowV1 {
    /// Construct one materialized setting row.
    pub const fn new(id: EngineSettingIdV1, value: EngineSettingValueV1) -> Self {
        Self { id, value }
    }

    /// Stable setting id.
    pub const fn id(&self) -> EngineSettingIdV1 {
        self.id
    }

    /// Fully materialized value.
    pub const fn value(&self) -> &EngineSettingValueV1 {
        &self.value
    }
}

/// Fully materialized settings for one actual clip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineClipSettingsV1 {
    clip_name: String,
    settings: Vec<EngineSettingRowV1>,
}

impl EngineClipSettingsV1 {
    /// Construct one clip row and canonicalize its setting-id order.
    ///
    /// # Errors
    ///
    /// Returns [`EngineContractError`] for an oversized clip name or setting
    /// list, an invalid path value, or duplicate setting ids.
    pub fn new(
        clip_name: impl Into<String>,
        mut settings: Vec<EngineSettingRowV1>,
    ) -> Result<Self, EngineContractError> {
        settings.sort_by_key(|row| row.id.as_str());
        let row = Self {
            clip_name: clip_name.into(),
            settings,
        };
        row.validate(true)?;
        Ok(row)
    }

    /// Actual clip name supplied during input resolution.
    pub fn clip_name(&self) -> &str {
        &self.clip_name
    }

    /// Fully materialized values in stable-id order.
    pub fn settings(&self) -> &[EngineSettingRowV1] {
        &self.settings
    }

    /// Look up one setting value within this clip row.
    pub fn setting(&self, id: EngineSettingIdV1) -> Option<&EngineSettingValueV1> {
        self.settings
            .iter()
            .find(|row| row.id == id)
            .map(|row| &row.value)
    }

    fn validate(&self, require_order: bool) -> Result<(), EngineContractError> {
        validate_text("settings.clips.clip_name", &self.clip_name)?;
        validate_collection_len("settings.clips.settings", self.settings.len())?;
        validate_unique_order(
            "settings.clips.settings",
            &self.settings,
            |row| row.id.as_str(),
            require_order,
        )?;
        for row in &self.settings {
            validate_setting_value(&row.value)?;
        }
        Ok(())
    }

    fn retained_text_bytes(&self) -> Result<usize, EngineContractError> {
        let paths = self.settings.iter().filter_map(|row| match &row.value {
            EngineSettingValueV1::SourceTransformPath(path) => Some(path.len()),
            _ => None,
        });
        checked_sum(
            "settings retained text",
            [self.clip_name.len()].into_iter().chain(paths),
        )
    }
}

/// Fully materialized, registry-independent V1 engine settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedEngineSettingsV1 {
    schema: String,
    identity: InputIdentity,
    document_settings: Vec<EngineSettingRowV1>,
    clips: Vec<EngineClipSettingsV1>,
}

impl ResolvedEngineSettingsV1 {
    /// Construct fully materialized settings against one exact embedded profile.
    ///
    /// Repeated equal clip names are retained and identity-significant.
    ///
    /// # Errors
    ///
    /// Returns [`EngineContractError`] for a bound, ordering, domain, scope,
    /// applicability, or required-value violation.
    pub fn new(
        profile: &ResolvedEngineProfileV1,
        mut document_settings: Vec<EngineSettingRowV1>,
        mut clips: Vec<EngineClipSettingsV1>,
    ) -> Result<Self, EngineContractError> {
        profile.validate()?;
        document_settings.sort_by_key(|row| row.id.as_str());
        clips.sort_by(|left, right| left.clip_name.cmp(&right.clip_name));
        let mut settings = Self {
            schema: RESOLVED_ENGINE_SETTINGS_V1_ID.to_owned(),
            identity: InputIdentity::from_bytes(&[]),
            document_settings,
            clips,
        };
        settings.validate_structure(true)?;
        settings.validate_materialization(profile, false)?;
        settings.identity = settings.computed_identity(profile);
        Ok(settings)
    }

    /// Contract id carried in the `schema` field.
    pub fn contract_id(&self) -> &str {
        &self.schema
    }

    /// SHA-256 plus byte count of the unchanged #464 settings preimage.
    pub const fn settings_identity(&self) -> &InputIdentity {
        &self.identity
    }

    /// Stable-id-ordered fully materialized document settings.
    pub fn document_settings(&self) -> &[EngineSettingRowV1] {
        &self.document_settings
    }

    /// Lexical-name-ordered actual clip rows, retaining repeated equal names.
    pub fn clips(&self) -> &[EngineClipSettingsV1] {
        &self.clips
    }

    /// Look up one document setting.
    pub fn document_setting(&self, id: EngineSettingIdV1) -> Option<&EngineSettingValueV1> {
        self.document_settings
            .iter()
            .find(|row| row.id == id)
            .map(|row| &row.value)
    }

    /// Look up one clip row by its identity-significant ordinal and exact name.
    pub fn clip_row(&self, ordinal: usize, clip_name: &str) -> Option<&EngineClipSettingsV1> {
        self.clips
            .get(ordinal)
            .filter(|row| row.clip_name == clip_name)
    }

    /// Validate structure, materialization, and identity against one profile.
    ///
    /// # Errors
    ///
    /// Returns [`EngineContractError`] for any invalid wire, cross-reference,
    /// or canonical identity.
    pub fn validate_against(
        &self,
        profile: &ResolvedEngineProfileV1,
    ) -> Result<(), EngineContractError> {
        profile.validate()?;
        self.validate_structure(true)?;
        self.validate_materialization(profile, true)
    }

    /// Append the complete unchanged #464 settings preimage, including its domain.
    pub(crate) fn encode_preimage(
        &self,
        profile: &ResolvedEngineProfileV1,
        encoder: &mut CanonicalEncoder,
    ) {
        encoder.token(ENGINE_SETTINGS_PREIMAGE_DOMAIN);
        encode_profile_key(encoder, &profile.selection);
        encoder.field("fact_bundle_urn");
        encoder.token(&profile.fact_bundle_urn);
        encoder.field("document_settings");
        encoder.count(self.document_settings.len());
        for row in &self.document_settings {
            encoder.token(row.id.as_str());
            encode_setting_value(encoder, &row.value);
        }
        encoder.field("clips");
        encoder.count(self.clips.len());
        for clip in &self.clips {
            encoder.token(&clip.clip_name);
            encoder.count(clip.settings.len());
            for row in &clip.settings {
                encoder.token(row.id.as_str());
                encode_setting_value(encoder, &row.value);
            }
        }
    }

    pub(crate) fn retained_rows(&self) -> Result<usize, EngineContractError> {
        checked_sum(
            "settings retained rows",
            [self.document_settings.len(), self.clips.len()]
                .into_iter()
                .chain(self.clips.iter().map(|clip| clip.settings.len())),
        )
    }

    pub(crate) fn retained_text_bytes(&self) -> Result<usize, EngineContractError> {
        let document_paths = self
            .document_settings
            .iter()
            .filter_map(|row| match &row.value {
                EngineSettingValueV1::SourceTransformPath(path) => Some(path.len()),
                _ => None,
            });
        checked_sum_results(
            "settings retained text",
            document_paths,
            self.clips
                .iter()
                .map(EngineClipSettingsV1::retained_text_bytes),
        )
    }

    fn computed_identity(&self, profile: &ResolvedEngineProfileV1) -> InputIdentity {
        let mut encoder = CanonicalEncoder::default();
        self.encode_preimage(profile, &mut encoder);
        encoder.identity()
    }

    fn validate_structure(&self, require_order: bool) -> Result<(), EngineContractError> {
        validate_schema(
            "settings.schema",
            &self.schema,
            RESOLVED_ENGINE_SETTINGS_V1_ID,
        )?;
        validate_collection_len("settings.document_settings", self.document_settings.len())?;
        validate_collection_len("settings.clips", self.clips.len())?;
        validate_unique_order(
            "settings.document_settings",
            &self.document_settings,
            |row| row.id.as_str(),
            require_order,
        )?;
        for row in &self.document_settings {
            validate_setting_value(&row.value)?;
        }
        if require_order
            && !self
                .clips
                .windows(2)
                .all(|pair| pair[0].clip_name <= pair[1].clip_name)
        {
            return Err(EngineContractError::NonCanonicalOrder {
                field: "settings.clips",
            });
        }
        for clip in &self.clips {
            clip.validate(require_order)?;
        }
        let rows = self.retained_rows()?;
        if rows > ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS {
            return Err(EngineContractError::TooManyAggregateRows {
                found: rows,
                max: ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS,
            });
        }
        let text = self.retained_text_bytes()?;
        if text > ENGINE_CONTRACT_V1_MAX_TOTAL_TEXT_BYTES {
            return Err(EngineContractError::TooMuchAggregateText {
                found: text,
                max: ENGINE_CONTRACT_V1_MAX_TOTAL_TEXT_BYTES,
            });
        }
        Ok(())
    }

    fn validate_materialization(
        &self,
        profile: &ResolvedEngineProfileV1,
        verify_identity: bool,
    ) -> Result<(), EngineContractError> {
        validate_rows_for_scope(
            profile,
            &self.document_settings,
            EngineSettingScopeV1::Document,
            "document",
        )?;
        for (ordinal, clip) in self.clips.iter().enumerate() {
            validate_rows_for_scope(
                profile,
                &clip.settings,
                EngineSettingScopeV1::Clip,
                &format!("clip[{ordinal}]"),
            )?;
        }
        if verify_identity && self.identity != self.computed_identity(profile) {
            return Err(EngineContractError::IdentityMismatch {
                contract: RESOLVED_ENGINE_SETTINGS_V1_ID,
            });
        }
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolvedEngineSettingsWireV1 {
    schema: String,
    identity: InputIdentity,
    document_settings: Vec<EngineSettingRowV1>,
    clips: Vec<EngineClipSettingsV1>,
}

impl ResolvedEngineSettingsV1 {
    fn from_wire(wire: ResolvedEngineSettingsWireV1) -> Result<Self, EngineContractError> {
        let settings = Self {
            schema: wire.schema,
            identity: wire.identity,
            document_settings: wire.document_settings,
            clips: wire.clips,
        };
        settings.validate_structure(true)?;
        Ok(settings)
    }
}

pub(crate) fn decode_resolved_engine_settings_v1(
    raw: &str,
) -> Result<ResolvedEngineSettingsV1, EngineContractDecodeError> {
    let wire = serde_json::from_str(raw).map_err(EngineContractDecodeError::Shape)?;
    ResolvedEngineSettingsV1::from_wire(wire).map_err(EngineContractDecodeError::Semantic)
}

impl<'de> Deserialize<'de> for ResolvedEngineSettingsV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::from_wire(ResolvedEngineSettingsWireV1::deserialize(deserializer)?)
            .map_err(D::Error::custom)
    }
}

/// Typed violation of the core-owned profile/settings contract.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EngineContractError {
    /// A contract schema id did not match its immutable V1 value.
    #[error("{field} must be {expected:?}, found {found:?}")]
    InvalidSchema {
        /// Field carrying the invalid schema id.
        field: &'static str,
        /// Required immutable schema id.
        expected: &'static str,
        /// Supplied schema id.
        found: String,
    },
    /// A required retained string was empty.
    #[error("{field} must not be empty")]
    EmptyText {
        /// Invalid field.
        field: &'static str,
    },
    /// One retained string exceeded the per-value byte limit.
    #[error("{field} retains {found} UTF-8 bytes, exceeding {max}")]
    TextTooLong {
        /// Oversized field.
        field: &'static str,
        /// Observed UTF-8 byte count.
        found: usize,
        /// Maximum permitted byte count.
        max: usize,
    },
    /// One collection exceeded the V1 row limit.
    #[error("{field} contains {found} rows, exceeding {max}")]
    TooManyRows {
        /// Oversized collection.
        field: &'static str,
        /// Observed row count.
        found: usize,
        /// Maximum permitted row count.
        max: usize,
    },
    /// Aggregate profile/settings rows exceeded the V1 limit.
    #[error("profile/settings retain {found} aggregate rows, exceeding {max}")]
    TooManyAggregateRows {
        /// Observed aggregate row count.
        found: usize,
        /// Maximum permitted aggregate row count.
        max: usize,
    },
    /// Aggregate retained UTF-8 text exceeded the V1 limit.
    #[error("profile/settings retain {found} UTF-8 bytes, exceeding {max}")]
    TooMuchAggregateText {
        /// Observed aggregate UTF-8 byte count.
        found: usize,
        /// Maximum permitted aggregate byte count.
        max: usize,
    },
    /// Checked arithmetic overflowed while accounting bounded work.
    #[error("checked arithmetic overflow while accounting {field}")]
    ArithmeticOverflow {
        /// Counter that overflowed.
        field: &'static str,
    },
    /// A set/map-shaped collection contains a duplicate stable key.
    #[error("{field} contains duplicate key {key:?}")]
    DuplicateKey {
        /// Collection containing the duplicate.
        field: &'static str,
        /// Duplicate stable key.
        key: String,
    },
    /// A canonical collection is not in its required order.
    #[error("{field} is not in canonical order")]
    NonCanonicalOrder {
        /// Unordered collection.
        field: &'static str,
    },
    /// The complete closed fact-id inventory is absent or malformed.
    #[error("profile facts must contain every V1 fact id exactly once")]
    InvalidFactInventory,
    /// A known fact carries a value from another fact domain.
    #[error("profile fact {fact:?} carries an invalid known-value variant")]
    InvalidFactValue {
        /// Fact whose value is invalid.
        fact: EngineFactIdV1,
    },
    /// The accepted-input list is empty, duplicated, or noncanonical.
    #[error("profile accepted_inputs must be a nonempty canonical set")]
    InvalidAcceptedInputs,
    /// A descriptor's applicability and default state disagree.
    #[error("setting descriptor {setting:?} has inconsistent applicability/default status")]
    InvalidDescriptorDefault {
        /// Invalid setting descriptor.
        setting: EngineSettingIdV1,
    },
    /// A source references a fact absent from the profile.
    #[error("primary source {source_id:?} references absent fact {fact:?}")]
    UnknownSourceFact {
        /// Source id.
        source_id: String,
        /// Missing fact id.
        fact: EngineFactIdV1,
    },
    /// A source cites a fact whose state is not known.
    #[error("primary source {source_id:?} references non-known fact {fact:?}")]
    SourceReferencesNonKnownFact {
        /// Source id.
        source_id: String,
        /// Non-known fact id.
        fact: EngineFactIdV1,
    },
    /// A source references a descriptor absent from the profile.
    #[error("primary source {source_id:?} references absent setting {setting:?}")]
    UnknownSourceSetting {
        /// Source id.
        source_id: String,
        /// Missing setting id.
        setting: EngineSettingIdV1,
    },
    /// No primary source supports a known profile fact.
    #[error("known profile fact {fact:?} has no primary-source reference")]
    UnreferencedKnownFact {
        /// Unsupported known fact.
        fact: EngineFactIdV1,
    },
    /// No primary source supports a setting descriptor.
    #[error("setting descriptor {setting:?} has no primary-source reference")]
    UnreferencedSetting {
        /// Unsupported descriptor.
        setting: EngineSettingIdV1,
    },
    /// A source-transform path is malformed.
    #[error("source-transform path is invalid: {reason}")]
    InvalidSourceTransformPath {
        /// Stable explanation of the malformed path.
        reason: &'static str,
    },
    /// A materialized setting is absent from the profile.
    #[error("{location} contains unknown setting {setting:?}")]
    UnknownMaterializedSetting {
        /// Stable document or clip location.
        location: String,
        /// Unknown setting id.
        setting: EngineSettingIdV1,
    },
    /// A materialized setting is declared at the wrong scope.
    #[error("{location} contains {setting:?} at the wrong scope")]
    WrongSettingScope {
        /// Stable document or clip location.
        location: String,
        /// Wrong-scope setting id.
        setting: EngineSettingIdV1,
    },
    /// A materialized setting is not applicable to the profile.
    #[error("{location} contains non-applicable setting {setting:?}")]
    NonApplicableSetting {
        /// Stable document or clip location.
        location: String,
        /// Non-applicable setting id.
        setting: EngineSettingIdV1,
    },
    /// A materialized value does not match its descriptor domain.
    #[error("{location} contains {setting:?} with a value outside its domain")]
    WrongSettingDomain {
        /// Stable document or clip location.
        location: String,
        /// Invalid setting id.
        setting: EngineSettingIdV1,
    },
    /// A required-without-default setting is absent.
    #[error("{location} is missing required setting {setting:?}")]
    MissingRequiredSetting {
        /// Stable document or clip location.
        location: String,
        /// Missing setting id.
        setting: EngineSettingIdV1,
    },
    /// A canonical identity does not match its semantic preimage.
    #[error("identity does not match canonical {contract}")]
    IdentityMismatch {
        /// Contract whose digest mismatched.
        contract: &'static str,
    },
}

fn validate_schema(
    field: &'static str,
    found: &str,
    expected: &'static str,
) -> Result<(), EngineContractError> {
    if found == expected {
        Ok(())
    } else {
        Err(EngineContractError::InvalidSchema {
            field,
            expected,
            found: found.to_owned(),
        })
    }
}

fn validate_required_text(field: &'static str, value: &str) -> Result<(), EngineContractError> {
    if value.is_empty() {
        return Err(EngineContractError::EmptyText { field });
    }
    validate_text(field, value)
}

fn validate_text(field: &'static str, value: &str) -> Result<(), EngineContractError> {
    if value.len() > ENGINE_CONTRACT_V1_MAX_TEXT_BYTES {
        Err(EngineContractError::TextTooLong {
            field,
            found: value.len(),
            max: ENGINE_CONTRACT_V1_MAX_TEXT_BYTES,
        })
    } else {
        Ok(())
    }
}

fn validate_collection_len(field: &'static str, found: usize) -> Result<(), EngineContractError> {
    if found > ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS {
        Err(EngineContractError::TooManyRows {
            field,
            found,
            max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
        })
    } else {
        Ok(())
    }
}

fn validate_unique_order<T>(
    field: &'static str,
    rows: &[T],
    key: impl Fn(&T) -> &str,
    require_order: bool,
) -> Result<(), EngineContractError> {
    let mut seen = BTreeSet::new();
    let mut previous = None;
    for row in rows {
        let current = key(row);
        if !seen.insert(current) {
            return Err(EngineContractError::DuplicateKey {
                field,
                key: current.to_owned(),
            });
        }
        if require_order && previous.is_some_and(|previous| previous >= current) {
            return Err(EngineContractError::NonCanonicalOrder { field });
        }
        previous = Some(current);
    }
    Ok(())
}

fn validate_fact_value(fact: &EngineProfileFactV1) -> Result<(), EngineContractError> {
    let EngineFactStateV1::Known(value) = &fact.state else {
        return Ok(());
    };
    let valid = matches!(
        (fact.id, value),
        (
            EngineFactIdV1::AcceptedInputs,
            EngineFactValueV1::AcceptedFormats(_)
        ) | (
            EngineFactIdV1::AnimationAddressability,
            EngineFactValueV1::AnimationAddressability(_)
        ) | (
            EngineFactIdV1::TargetCoordinateBasis,
            EngineFactValueV1::CoordinateBasis(_)
        ) | (
            EngineFactIdV1::TargetLinearUnit,
            EngineFactValueV1::LinearUnit(_)
        ) | (
            EngineFactIdV1::UnitConversionControl | EngineFactIdV1::AxisConversionControl,
            EngineFactValueV1::ConversionControl(_)
        ) | (
            EngineFactIdV1::WholeEndFrameRequired,
            EngineFactValueV1::Boolean(_)
        ) | (
            EngineFactIdV1::AnimationChannelHandling
                | EngineFactIdV1::ExtensionHandling
                | EngineFactIdV1::ConstructHandling,
            EngineFactValueV1::ImportHandling(_)
        ) | (
            EngineFactIdV1::AnimationTargetAddressability,
            EngineFactValueV1::TargetAddressability(_)
        ) | (
            EngineFactIdV1::RootMotionAddressability,
            EngineFactValueV1::RootMotionAddressability(_)
        )
    );
    if !valid {
        return Err(EngineContractError::InvalidFactValue { fact: fact.id });
    }
    if let EngineFactValueV1::AcceptedFormats(formats) = value
        && (formats.is_empty()
            || formats.len() > ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS
            || !formats
                .windows(2)
                .all(|pair| source_format_name(pair[0]) < source_format_name(pair[1])))
    {
        return Err(EngineContractError::InvalidAcceptedInputs);
    }
    if let EngineFactValueV1::ConversionControl(EngineConversionControlV1::ProfileSetting(
        setting,
    )) = value
    {
        let expected = match fact.id {
            EngineFactIdV1::UnitConversionControl => EngineSettingIdV1::ConvertUnits,
            EngineFactIdV1::AxisConversionControl => EngineSettingIdV1::BakeAxisConversion,
            _ => return Err(EngineContractError::InvalidFactValue { fact: fact.id }),
        };
        if *setting != expected {
            return Err(EngineContractError::InvalidFactValue { fact: fact.id });
        }
    }
    Ok(())
}

fn validate_setting_value(value: &EngineSettingValueV1) -> Result<(), EngineContractError> {
    let EngineSettingValueV1::SourceTransformPath(path) = value else {
        return Ok(());
    };
    validate_text("source_transform_path", path)?;
    let reason = if path.is_empty() {
        Some("empty path")
    } else if path.starts_with('/') {
        Some("absolute path")
    } else if path.chars().any(char::is_control) {
        Some("control character")
    } else if path.split('/').any(str::is_empty) {
        Some("empty path segment")
    } else if path.split('/').any(|segment| matches!(segment, "." | "..")) {
        Some("dot path segment")
    } else {
        None
    };
    if let Some(reason) = reason {
        Err(EngineContractError::InvalidSourceTransformPath { reason })
    } else {
        Ok(())
    }
}

fn validate_rows_for_scope(
    profile: &ResolvedEngineProfileV1,
    rows: &[EngineSettingRowV1],
    scope: EngineSettingScopeV1,
    location: &str,
) -> Result<(), EngineContractError> {
    for row in rows {
        let Some(descriptor) = profile.setting_descriptor(row.id) else {
            return Err(EngineContractError::UnknownMaterializedSetting {
                location: location.to_owned(),
                setting: row.id,
            });
        };
        if descriptor.scope != scope {
            return Err(EngineContractError::WrongSettingScope {
                location: location.to_owned(),
                setting: row.id,
            });
        }
        if descriptor.applicability != EngineSettingApplicabilityV1::Applicable {
            return Err(EngineContractError::NonApplicableSetting {
                location: location.to_owned(),
                setting: row.id,
            });
        }
        let domain_matches = matches!(
            (descriptor.domain, &row.value),
            (
                EngineSettingDomainV1::Boolean,
                EngineSettingValueV1::Boolean(_)
            ) | (
                EngineSettingDomainV1::BakeOrExtract,
                EngineSettingValueV1::BakeOrExtract(_)
            ) | (
                EngineSettingDomainV1::SourceTransformPath,
                EngineSettingValueV1::SourceTransformPath(_)
            )
        );
        if !domain_matches {
            return Err(EngineContractError::WrongSettingDomain {
                location: location.to_owned(),
                setting: row.id,
            });
        }
    }
    for descriptor in &profile.setting_descriptors {
        if descriptor.scope == scope
            && descriptor.applicability == EngineSettingApplicabilityV1::Applicable
            && descriptor.default_status == EngineDefaultStatusV1::RequiredWithoutDefault
            && !rows.iter().any(|row| row.id == descriptor.id)
        {
            return Err(EngineContractError::MissingRequiredSetting {
                location: location.to_owned(),
                setting: descriptor.id,
            });
        }
    }
    Ok(())
}

fn checked_sum(
    field: &'static str,
    values: impl IntoIterator<Item = usize>,
) -> Result<usize, EngineContractError> {
    values.into_iter().try_fold(0_usize, |total, value| {
        total
            .checked_add(value)
            .ok_or(EngineContractError::ArithmeticOverflow { field })
    })
}

fn checked_sum_results(
    field: &'static str,
    initial: impl IntoIterator<Item = usize>,
    values: impl IntoIterator<Item = Result<usize, EngineContractError>>,
) -> Result<usize, EngineContractError> {
    let initial = checked_sum(field, initial)?;
    values.into_iter().try_fold(initial, |total, value| {
        total
            .checked_add(value?)
            .ok_or(EngineContractError::ArithmeticOverflow { field })
    })
}

fn encode_profile_key(encoder: &mut CanonicalEncoder, selection: &EngineProfileSelectionV1) {
    encoder.field("selection");
    encoder.token(&selection.family);
    encoder.token(selection.profile_revision.to_string());
    encoder.token(&selection.engine_version);
    encoder.token(&selection.importer);
}

fn encode_fact_state(encoder: &mut CanonicalEncoder, state: &EngineFactStateV1) {
    match state {
        EngineFactStateV1::Unknown => encoder.token("unknown"),
        EngineFactStateV1::NotApplicable => encoder.token("not_applicable"),
        EngineFactStateV1::Known(value) => {
            encoder.token("known");
            match value {
                EngineFactValueV1::AcceptedFormats(formats) => {
                    encoder.token("accepted_formats");
                    encoder.count(formats.len());
                    for format in formats {
                        encoder.token(source_format_name(*format));
                    }
                }
                EngineFactValueV1::AnimationAddressability(value) => {
                    encoder.token("animation_addressability");
                    encoder.token(match value {
                        EngineAnimationAddressabilityV1::GltfAssetLabel => "gltf_asset_label",
                    });
                }
                EngineFactValueV1::CoordinateBasis(value) => {
                    encoder.token("coordinate_basis");
                    encoder.token(match value.handedness {
                        EngineHandednessV1::Left => "left",
                        EngineHandednessV1::Right => "right",
                    });
                    encoder.token(match value.up_axis {
                        EngineUpAxisV1::X => "x",
                        EngineUpAxisV1::Y => "y",
                        EngineUpAxisV1::Z => "z",
                    });
                    encoder.token(match value.forward_axis {
                        EngineForwardAxisV1::PositiveX => "+x",
                        EngineForwardAxisV1::NegativeX => "-x",
                        EngineForwardAxisV1::PositiveY => "+y",
                        EngineForwardAxisV1::NegativeY => "-y",
                        EngineForwardAxisV1::PositiveZ => "+z",
                        EngineForwardAxisV1::NegativeZ => "-z",
                    });
                }
                EngineFactValueV1::LinearUnit(value) => {
                    encoder.token("linear_unit");
                    encoder.token(match value {
                        EngineLinearUnitV1::Metre => "metre",
                        EngineLinearUnitV1::Centimetre => "centimetre",
                    });
                }
                EngineFactValueV1::ConversionControl(value) => {
                    encoder.token("conversion_control");
                    match value {
                        EngineConversionControlV1::ProfileSetting(setting) => {
                            encoder.token("profile_setting");
                            encoder.token(setting.as_str());
                        }
                        EngineConversionControlV1::ImporterOption => {
                            encoder.token("importer_option");
                        }
                    }
                }
                EngineFactValueV1::Boolean(value) => {
                    encoder.token("boolean");
                    encoder.token(if *value { "true" } else { "false" });
                }
                EngineFactValueV1::ImportHandling(value) => {
                    encoder.token("import_handling");
                    encoder.token(match value {
                        EngineImportHandlingV1::Preserved => "preserved",
                        EngineImportHandlingV1::Converted => "converted",
                        EngineImportHandlingV1::Discarded => "discarded",
                        EngineImportHandlingV1::Unsupported => "unsupported",
                    });
                }
                EngineFactValueV1::TargetAddressability(value) => {
                    encoder.token("target_addressability");
                    encoder.token(match value {
                        EngineTargetAddressabilityV1::NamePathDerivedId => "name_path_derived_id",
                    });
                }
                EngineFactValueV1::RootMotionAddressability(value) => {
                    encoder.token("root_motion_addressability");
                    encoder.token(match value {
                        EngineRootMotionAddressabilityV1::ExactSourceTransformPath => {
                            "exact_source_transform_path"
                        }
                        EngineRootMotionAddressabilityV1::HumanoidAvatarBody => {
                            "humanoid_avatar_body"
                        }
                    });
                }
            }
        }
    }
}

fn encode_setting_value(encoder: &mut CanonicalEncoder, value: &EngineSettingValueV1) {
    match value {
        EngineSettingValueV1::Boolean(value) => {
            encoder.token("boolean");
            encoder.token(if *value { "true" } else { "false" });
        }
        EngineSettingValueV1::BakeOrExtract(value) => {
            encoder.token("bake_or_extract");
            encoder.token(match value {
                EngineBakeOrExtractV1::Bake => "bake",
                EngineBakeOrExtractV1::Extract => "extract",
            });
        }
        EngineSettingValueV1::SourceTransformPath(value) => {
            encoder.token("source_transform_path");
            encoder.token(value);
        }
    }
}

const fn source_format_name(format: SourceFormatV1) -> &'static str {
    match format {
        SourceFormatV1::GltfJson => "gltf_json",
        SourceFormatV1::Glb => "glb",
        SourceFormatV1::Fbx => "fbx",
    }
}

const fn setting_scope_name(scope: EngineSettingScopeV1) -> &'static str {
    match scope {
        EngineSettingScopeV1::Document => "document",
        EngineSettingScopeV1::Clip => "clip",
    }
}

const fn setting_domain_name(domain: EngineSettingDomainV1) -> &'static str {
    match domain {
        EngineSettingDomainV1::Boolean => "boolean",
        EngineSettingDomainV1::BakeOrExtract => "bake_or_extract",
        EngineSettingDomainV1::SourceTransformPath => "source_transform_path",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fact_inventory(accepted: Vec<SourceFormatV1>) -> Vec<EngineProfileFactV1> {
        ALL_FACT_IDS
            .into_iter()
            .map(|id| {
                let state = if id == EngineFactIdV1::AcceptedInputs {
                    EngineFactStateV1::Known(EngineFactValueV1::AcceptedFormats(accepted.clone()))
                } else {
                    EngineFactStateV1::Unknown
                };
                EngineProfileFactV1::new(id, state)
            })
            .collect()
    }

    fn godot_profile() -> ResolvedEngineProfileV1 {
        ResolvedEngineProfileV1::new(
            EngineProfileSelectionV1::new("godot", 1, "4.7", "resource-importer-scene").unwrap(),
            "urn:animsmith:engine-profile:godot:1",
            fact_inventory(vec![
                SourceFormatV1::GltfJson,
                SourceFormatV1::Glb,
                SourceFormatV1::Fbx,
            ]),
            vec![],
            vec![
                EnginePrimarySourceV1::new(
                    "godot-resource-importer-scene-4.7",
                    "4.7",
                    "https://docs.godotengine.org/en/4.7/classes/class_resourceimporterscene.html",
                    "2026-08-20",
                    vec![EngineFactIdV1::AcceptedInputs],
                    vec![],
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    fn settings_profile(family: &str) -> ResolvedEngineProfileV1 {
        let mut facts = fact_inventory(vec![SourceFormatV1::Fbx]);
        facts
            .iter_mut()
            .find(|fact| fact.id == EngineFactIdV1::UnitConversionControl)
            .unwrap()
            .state = EngineFactStateV1::Known(EngineFactValueV1::ConversionControl(
            EngineConversionControlV1::ProfileSetting(EngineSettingIdV1::ConvertUnits),
        ));
        facts
            .iter_mut()
            .find(|fact| fact.id == EngineFactIdV1::AxisConversionControl)
            .unwrap()
            .state = EngineFactStateV1::Known(EngineFactValueV1::ConversionControl(
            EngineConversionControlV1::ProfileSetting(EngineSettingIdV1::BakeAxisConversion),
        ));
        let descriptors = vec![
            EngineSettingDescriptorV1::new(
                EngineSettingIdV1::ConvertUnits,
                EngineSettingScopeV1::Document,
                EngineSettingDomainV1::Boolean,
                EngineSettingApplicabilityV1::Applicable,
                EngineDefaultStatusV1::RequiredWithoutDefault,
            ),
            EngineSettingDescriptorV1::new(
                EngineSettingIdV1::BakeAxisConversion,
                EngineSettingScopeV1::Document,
                EngineSettingDomainV1::Boolean,
                EngineSettingApplicabilityV1::Applicable,
                EngineDefaultStatusV1::RequiredWithoutDefault,
            ),
        ];
        let source = EnginePrimarySourceV1::new(
            "source",
            "1",
            "https://example.invalid/source",
            "2026-08-20",
            vec![
                EngineFactIdV1::AcceptedInputs,
                EngineFactIdV1::UnitConversionControl,
                EngineFactIdV1::AxisConversionControl,
            ],
            vec![
                EngineSettingIdV1::ConvertUnits,
                EngineSettingIdV1::BakeAxisConversion,
            ],
        )
        .unwrap();
        ResolvedEngineProfileV1::new(
            EngineProfileSelectionV1::new(family, 1, "1", "importer").unwrap(),
            format!("urn:animsmith:engine-profile:{family}:1"),
            facts,
            descriptors,
            vec![source],
        )
        .unwrap()
    }

    fn document_settings() -> Vec<EngineSettingRowV1> {
        vec![
            EngineSettingRowV1::new(
                EngineSettingIdV1::ConvertUnits,
                EngineSettingValueV1::Boolean(true),
            ),
            EngineSettingRowV1::new(
                EngineSettingIdV1::BakeAxisConversion,
                EngineSettingValueV1::Boolean(false),
            ),
        ]
    }

    #[test]
    fn profile_encoder_preserves_464_godot_golden() {
        let profile = godot_profile();
        assert_eq!(
            profile.facts_identity().sha256(),
            "e9c8316d1655c487b60dd35bbfc70289952c5fa12f4718f0be09c7e9a00fbe87"
        );
        assert_eq!(profile.facts_identity().bytes(), 1_166);

        let mut encoder = CanonicalEncoder::default();
        profile.encode_preimage(&mut encoder);
        assert_eq!(encoder.into_bytes().len(), 1_166);
    }

    #[test]
    fn settings_encoder_preserves_464_godot_golden() {
        let profile = godot_profile();
        let settings = ResolvedEngineSettingsV1::new(&profile, vec![], vec![]).unwrap();
        assert_eq!(
            settings.settings_identity().sha256(),
            "02032c315fa41ad65249efe1b6914456b3b98caf9b5374b168854cd357f85515"
        );
        assert_eq!(settings.settings_identity().bytes(), 240);
    }

    #[test]
    fn constructors_canonicalize_sets_maps_and_retain_repeated_clips() {
        let profile = settings_profile("test");
        let first = ResolvedEngineSettingsV1::new(
            &profile,
            document_settings(),
            vec![
                EngineClipSettingsV1::new("walk", vec![]).unwrap(),
                EngineClipSettingsV1::new("idle", vec![]).unwrap(),
                EngineClipSettingsV1::new("walk", vec![]).unwrap(),
            ],
        )
        .unwrap();
        let mut reversed = document_settings();
        reversed.reverse();
        let second = ResolvedEngineSettingsV1::new(
            &profile,
            reversed,
            vec![
                EngineClipSettingsV1::new("walk", vec![]).unwrap(),
                EngineClipSettingsV1::new("walk", vec![]).unwrap(),
                EngineClipSettingsV1::new("idle", vec![]).unwrap(),
            ],
        )
        .unwrap();

        assert_eq!(first, second);
        assert_eq!(
            first
                .clips()
                .iter()
                .map(EngineClipSettingsV1::clip_name)
                .collect::<Vec<_>>(),
            vec!["idle", "walk", "walk"]
        );
        assert!(first.clip_row(1, "walk").is_some());
        assert!(first.clip_row(1, "idle").is_none());

        let deduplicated = ResolvedEngineSettingsV1::new(
            &profile,
            document_settings(),
            vec![
                EngineClipSettingsV1::new("idle", vec![]).unwrap(),
                EngineClipSettingsV1::new("walk", vec![]).unwrap(),
            ],
        )
        .unwrap();
        assert_ne!(first.settings_identity(), deduplicated.settings_identity());
    }

    #[test]
    fn wire_round_trip_is_strict_and_revalidates_identities() {
        let profile = godot_profile();
        let value = serde_json::to_value(&profile).unwrap();
        assert_eq!(
            value["schema"],
            json!("urn:animsmith:engine-profile-facts:1")
        );
        assert!(value.get("primary_sources").is_some());
        let decoded: ResolvedEngineProfileV1 = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(decoded, profile);

        let mut unknown = value.clone();
        unknown["unexpected"] = json!(true);
        assert!(
            serde_json::from_value::<ResolvedEngineProfileV1>(unknown)
                .unwrap_err()
                .to_string()
                .contains("unknown field")
        );

        let mut identity = value.clone();
        identity["identity"]["bytes"] = json!(0);
        assert!(
            serde_json::from_value::<ResolvedEngineProfileV1>(identity)
                .unwrap_err()
                .to_string()
                .contains("identity does not match")
        );

        let mut reordered = value;
        reordered["facts"].as_array_mut().unwrap().swap(0, 1);
        assert!(
            serde_json::from_value::<ResolvedEngineProfileV1>(reordered)
                .unwrap_err()
                .to_string()
                .contains("canonical order")
        );
    }

    #[test]
    fn profile_mutations_return_the_specific_first_contract_error() {
        let profile = godot_profile();

        let mut changed = profile.clone();
        changed.schema = "urn:changed".into();
        assert!(matches!(
            changed.validate(),
            Err(EngineContractError::InvalidSchema {
                field: "profile.schema",
                ..
            })
        ));

        let mut changed = profile.clone();
        changed.selection.family.push_str("-changed");
        assert_eq!(
            changed.validate(),
            Err(EngineContractError::IdentityMismatch {
                contract: ENGINE_PROFILE_FACTS_V1_ID,
            })
        );

        let mut changed = profile.clone();
        changed.facts[0].state = EngineFactStateV1::Unknown;
        assert_eq!(
            changed.validate(),
            Err(EngineContractError::InvalidAcceptedInputs)
        );

        let mut changed = profile;
        changed.primary_sources[0].url.push_str("/changed");
        assert_eq!(
            changed.validate(),
            Err(EngineContractError::IdentityMismatch {
                contract: ENGINE_PROFILE_FACTS_V1_ID,
            })
        );
    }

    #[test]
    fn profile_acceptance_mutation_matrix_pins_tuple_facts_descriptors_and_sources() {
        let profile = settings_profile("matrix");
        let identity_mismatch = Err(EngineContractError::IdentityMismatch {
            contract: ENGINE_PROFILE_FACTS_V1_ID,
        });

        let mut changed = profile.clone();
        changed.selection.family.push_str("-changed");
        assert_eq!(changed.validate(), identity_mismatch);

        let mut changed = profile.clone();
        changed.selection.profile_revision += 1;
        assert_eq!(changed.validate(), identity_mismatch);

        let mut changed = profile.clone();
        changed.selection.engine_version.push_str("-changed");
        assert_eq!(changed.validate(), identity_mismatch);

        let mut changed = profile.clone();
        changed.selection.importer.push_str("-changed");
        assert_eq!(changed.validate(), identity_mismatch);

        let mut changed = profile.clone();
        changed.fact_bundle_urn.push_str(":changed");
        assert_eq!(changed.validate(), identity_mismatch);

        let mut changed = profile.clone();
        changed.facts.pop();
        assert_eq!(
            changed.validate(),
            Err(EngineContractError::InvalidFactInventory)
        );

        let mut changed = profile.clone();
        changed
            .facts
            .iter_mut()
            .find(|fact| fact.id == EngineFactIdV1::AcceptedInputs)
            .unwrap()
            .state = EngineFactStateV1::Known(EngineFactValueV1::Boolean(true));
        assert_eq!(
            changed.validate(),
            Err(EngineContractError::InvalidFactValue {
                fact: EngineFactIdV1::AcceptedInputs,
            })
        );

        let mut changed = profile.clone();
        changed.setting_descriptors[1].id = EngineSettingIdV1::BakeAxisConversion;
        assert_eq!(
            changed.validate(),
            Err(EngineContractError::InvalidFactValue {
                fact: EngineFactIdV1::UnitConversionControl,
            })
        );

        let mut changed = profile.clone();
        changed.setting_descriptors[0].scope = EngineSettingScopeV1::Clip;
        assert_eq!(changed.validate(), identity_mismatch);

        let mut changed = profile.clone();
        changed.setting_descriptors[0].domain = EngineSettingDomainV1::BakeOrExtract;
        assert_eq!(changed.validate(), identity_mismatch);

        let mut changed = profile.clone();
        let descriptor_id = changed.setting_descriptors[0].id;
        changed.setting_descriptors[0].applicability = EngineSettingApplicabilityV1::NotApplicable;
        assert_eq!(
            changed.validate(),
            Err(EngineContractError::InvalidDescriptorDefault {
                setting: descriptor_id,
            })
        );

        let mut changed = profile.clone();
        let descriptor_id = changed.setting_descriptors[0].id;
        changed.setting_descriptors[0].default_status = EngineDefaultStatusV1::NotApplicable;
        assert_eq!(
            changed.validate(),
            Err(EngineContractError::InvalidDescriptorDefault {
                setting: descriptor_id,
            })
        );

        let mut changed = profile.clone();
        changed.primary_sources[0].id.clear();
        assert_eq!(
            changed.validate(),
            Err(EngineContractError::EmptyText {
                field: "primary_sources.id",
            })
        );

        let mut changed = profile.clone();
        changed.primary_sources[0].url.push_str("/changed");
        assert_eq!(changed.validate(), identity_mismatch);

        let mut changed = profile.clone();
        changed.primary_sources[0]
            .supported_fact_ids
            .push(EngineFactIdV1::AnimationAddressability);
        changed.primary_sources[0]
            .supported_fact_ids
            .sort_by_key(|id| id.as_str());
        assert_eq!(
            changed.validate(),
            Err(EngineContractError::SourceReferencesNonKnownFact {
                source_id: "source".to_owned(),
                fact: EngineFactIdV1::AnimationAddressability,
            })
        );

        let mut changed = profile.clone();
        changed.schema = "urn:changed".to_owned();
        assert_eq!(
            changed.validate(),
            Err(EngineContractError::InvalidSchema {
                field: "profile.schema",
                expected: ENGINE_PROFILE_FACTS_V1_ID,
                found: "urn:changed".to_owned(),
            })
        );

        let mut changed = profile;
        changed.identity = InputIdentity::from_bytes(b"changed");
        assert_eq!(changed.validate(), identity_mismatch);
    }

    #[test]
    fn settings_wire_requires_profile_validation_after_structural_read() {
        let profile = settings_profile("wire");
        let settings =
            ResolvedEngineSettingsV1::new(&profile, document_settings(), vec![]).unwrap();
        let mut value = serde_json::to_value(&settings).unwrap();
        let decoded: ResolvedEngineSettingsV1 = serde_json::from_value(value.clone()).unwrap();
        decoded.validate_against(&profile).unwrap();

        value["identity"]["bytes"] = json!(0);
        let decoded: ResolvedEngineSettingsV1 = serde_json::from_value(value).unwrap();
        assert_eq!(
            decoded.validate_against(&profile),
            Err(EngineContractError::IdentityMismatch {
                contract: RESOLVED_ENGINE_SETTINGS_V1_ID,
            })
        );
    }

    #[test]
    fn settings_mutations_reject_noncanonical_order_before_identity() {
        let profile = settings_profile("order");
        let mut settings =
            ResolvedEngineSettingsV1::new(&profile, document_settings(), vec![]).unwrap();
        settings.document_settings.swap(0, 1);
        assert_eq!(
            settings.validate_against(&profile),
            Err(EngineContractError::NonCanonicalOrder {
                field: "settings.document_settings",
            })
        );
    }

    #[test]
    fn materialized_settings_acceptance_mutation_matrix_pins_id_value_location_and_identity() {
        let profile = settings_profile("settings-matrix");
        let settings =
            ResolvedEngineSettingsV1::new(&profile, document_settings(), vec![]).unwrap();

        let mut changed = settings.clone();
        changed.document_settings[1].id = EngineSettingIdV1::RootMotionSource;
        assert_eq!(
            changed.validate_against(&profile),
            Err(EngineContractError::UnknownMaterializedSetting {
                location: "document".to_owned(),
                setting: EngineSettingIdV1::RootMotionSource,
            })
        );

        let mut changed = settings.clone();
        changed.document_settings[0].value =
            EngineSettingValueV1::BakeOrExtract(EngineBakeOrExtractV1::Bake);
        assert_eq!(
            changed.validate_against(&profile),
            Err(EngineContractError::WrongSettingDomain {
                location: "document".to_owned(),
                setting: EngineSettingIdV1::BakeAxisConversion,
            })
        );

        let mut changed = settings.clone();
        changed.clips.push(
            EngineClipSettingsV1::new(
                "walk",
                vec![EngineSettingRowV1::new(
                    EngineSettingIdV1::ConvertUnits,
                    EngineSettingValueV1::Boolean(true),
                )],
            )
            .unwrap(),
        );
        assert_eq!(
            changed.validate_against(&profile),
            Err(EngineContractError::WrongSettingScope {
                location: "clip[0]".to_owned(),
                setting: EngineSettingIdV1::ConvertUnits,
            })
        );

        let mut changed = settings.clone();
        changed.document_settings.swap(0, 1);
        assert_eq!(
            changed.validate_against(&profile),
            Err(EngineContractError::NonCanonicalOrder {
                field: "settings.document_settings",
            })
        );

        let mut changed = settings.clone();
        changed.schema = "urn:changed".to_owned();
        assert_eq!(
            changed.validate_against(&profile),
            Err(EngineContractError::InvalidSchema {
                field: "settings.schema",
                expected: RESOLVED_ENGINE_SETTINGS_V1_ID,
                found: "urn:changed".to_owned(),
            })
        );

        let mut changed = settings;
        changed.identity = InputIdentity::from_bytes(b"changed");
        assert_eq!(
            changed.validate_against(&profile),
            Err(EngineContractError::IdentityMismatch {
                contract: RESOLVED_ENGINE_SETTINGS_V1_ID,
            })
        );
    }

    #[test]
    fn clip_collection_bound_accepts_exact_n_and_rejects_n_plus_one() {
        let profile = godot_profile();
        let clips = (0..ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS)
            .map(|_| EngineClipSettingsV1::new("same", vec![]).unwrap())
            .collect();
        let exact = ResolvedEngineSettingsV1::new(&profile, vec![], clips).unwrap();
        assert_eq!(exact.clips().len(), ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS);

        let clips = (0..=ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS)
            .map(|_| EngineClipSettingsV1::new("same", vec![]).unwrap())
            .collect();
        assert_eq!(
            ResolvedEngineSettingsV1::new(&profile, vec![], clips),
            Err(EngineContractError::TooManyRows {
                field: "settings.clips",
                found: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
            })
        );
    }

    #[test]
    fn text_bound_accepts_exact_n_and_rejects_n_plus_one() {
        let exact = "a".repeat(ENGINE_CONTRACT_V1_MAX_TEXT_BYTES);
        EngineClipSettingsV1::new(
            "clip",
            vec![EngineSettingRowV1::new(
                EngineSettingIdV1::RootMotionSource,
                EngineSettingValueV1::SourceTransformPath(exact),
            )],
        )
        .unwrap();

        let oversized = "a".repeat(ENGINE_CONTRACT_V1_MAX_TEXT_BYTES + 1);
        assert_eq!(
            EngineClipSettingsV1::new(
                "clip",
                vec![EngineSettingRowV1::new(
                    EngineSettingIdV1::RootMotionSource,
                    EngineSettingValueV1::SourceTransformPath(oversized),
                )],
            ),
            Err(EngineContractError::TextTooLong {
                field: "source_transform_path",
                found: ENGINE_CONTRACT_V1_MAX_TEXT_BYTES + 1,
                max: ENGINE_CONTRACT_V1_MAX_TEXT_BYTES,
            })
        );
    }

    #[test]
    fn materialized_setting_mutations_name_the_violated_contract() {
        let profile = settings_profile("mutations");
        assert_eq!(
            ResolvedEngineSettingsV1::new(
                &profile,
                vec![EngineSettingRowV1::new(
                    EngineSettingIdV1::ConvertUnits,
                    EngineSettingValueV1::Boolean(true),
                )],
                vec![],
            ),
            Err(EngineContractError::MissingRequiredSetting {
                location: "document".into(),
                setting: EngineSettingIdV1::BakeAxisConversion,
            })
        );
        assert_eq!(
            ResolvedEngineSettingsV1::new(
                &profile,
                vec![
                    EngineSettingRowV1::new(
                        EngineSettingIdV1::ConvertUnits,
                        EngineSettingValueV1::BakeOrExtract(EngineBakeOrExtractV1::Bake),
                    ),
                    EngineSettingRowV1::new(
                        EngineSettingIdV1::BakeAxisConversion,
                        EngineSettingValueV1::Boolean(true),
                    ),
                ],
                vec![],
            ),
            Err(EngineContractError::WrongSettingDomain {
                location: "document".into(),
                setting: EngineSettingIdV1::ConvertUnits,
            })
        );
    }

    #[test]
    fn source_format_and_input_identity_deserialization_are_closed() {
        assert_eq!(
            serde_json::from_str::<SourceFormatV1>("\"glb\"").unwrap(),
            SourceFormatV1::Glb
        );
        assert!(serde_json::from_str::<SourceFormatV1>("\"obj\"").is_err());

        let identity = InputIdentity::from_bytes(b"identity");
        let wire = serde_json::to_string(&identity).unwrap();
        assert_eq!(
            serde_json::from_str::<InputIdentity>(&wire).unwrap(),
            identity
        );
        let mut upper = serde_json::to_value(&identity).unwrap();
        upper["sha256"] = json!("A".repeat(64));
        assert!(serde_json::from_value::<InputIdentity>(upper).is_err());
    }

    #[test]
    fn canonical_encoder_uses_length_prefixed_tokens() {
        let mut encoder = CanonicalEncoder::new("domain");
        encoder.field("field");
        encoder.count(12);
        encode_input_identity(&mut encoder, &InputIdentity::from_bytes(b"x"));
        let bytes = encoder.into_bytes();
        assert_eq!(&bytes[..8], &6_u64.to_be_bytes());
        assert_eq!(&bytes[8..14], b"domain");
    }
}
