//! Registry-independent engine profile facts and resolved settings.
//!
//! These wire values deliberately mirror the closed V1 vocabulary owned by
//! `animsmith-engine` without making core depend on that crate. Their
//! canonical encoders preserve the byte preimages introduced with the V1
//! engine registry.

use crate::bounded_deserialize::{
    BudgetedCappedSequenceSeed, CappedSequence, RowBudget, consume_ignored_tail,
    deserialize_capped_sequence,
};
use crate::{InputIdentity, SourceFormatV1};
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use std::collections::BTreeSet;
use std::fmt;
use std::marker::PhantomData;

/// Semantic contract for a self-contained V1 engine profile record.
pub const ENGINE_PROFILE_FACTS_V1_ID: &str = "urn:animsmith:engine-profile-facts:1";
/// Semantic contract for the extended immutable engine-profile vocabulary.
pub const ENGINE_PROFILE_FACTS_V2_ID: &str = "urn:animsmith:engine-profile-facts:2";
/// Semantic contract for fully materialized V1 engine settings.
pub const RESOLVED_ENGINE_SETTINGS_V1_ID: &str = "urn:animsmith:resolved-engine-settings:1";
/// Semantic contract for bounded, explicitly partial V2 engine settings.
pub const RESOLVED_ENGINE_SETTINGS_V2_ID: &str = "urn:animsmith:resolved-engine-settings:2";
/// Semantic contract for origin-bearing resolved engine settings.
pub const RESOLVED_ENGINE_SETTINGS_V3_ID: &str = "urn:animsmith:resolved-engine-settings:3";
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
const ENGINE_FACTS_V2_PREIMAGE_DOMAIN: &str = "animsmith-engine-facts-v2";
const ENGINE_SETTINGS_V3_PREIMAGE_DOMAIN: &str = "animsmith-engine-settings-v3";

fn deserialize_collection_rows<'de, D, T>(deserializer: D) -> Result<CappedSequence<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    deserialize_capped_sequence(deserializer, ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS)
}

fn deserialize_collection_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let values = deserialize_capped_sequence(deserializer, ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS)?;
    if values.overflowed {
        return Err(D::Error::custom(
            "engine-contract collection exceeds 4096 rows",
        ));
    }
    Ok(values.values)
}

#[derive(Debug)]
struct ProfileRows {
    local: RowBudget,
    provenance: Option<RowBudget>,
}

impl ProfileRows {
    fn new(provenance_limit: Option<usize>) -> Self {
        Self {
            local: RowBudget::new(ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS),
            provenance: provenance_limit.map(RowBudget::new),
        }
    }

    fn admit_top_level(&mut self) -> bool {
        if !self.local.admit() {
            return false;
        }
        self.provenance.as_mut().is_none_or(RowBudget::admit)
    }

    fn provenance_overflowed(&self) -> bool {
        self.provenance.as_ref().is_some_and(RowBudget::overflowed)
    }
}

#[derive(Debug)]
struct SettingsRows {
    local: RowBudget,
    provenance: Option<RowBudget>,
}

impl SettingsRows {
    fn new(provenance_limit: Option<usize>) -> Self {
        Self {
            local: RowBudget::new(ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS),
            provenance: provenance_limit.map(RowBudget::new),
        }
    }

    fn admit_clip(&mut self) -> bool {
        self.local.admit()
    }

    fn admit_setting(&mut self) -> bool {
        if !self.local.admit() {
            return false;
        }
        self.provenance.as_mut().is_none_or(RowBudget::admit)
    }

    fn provenance_overflowed(&self) -> bool {
        self.provenance.as_ref().is_some_and(RowBudget::overflowed)
    }
}

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
    /// Bevy addresses each glTF animation by its source-array index through
    /// `GltfAssetLabel::Animation(index)`; animation names populate Bevy's
    /// separate named-animation map rather than this typed label.
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineFactValueV1 {
    /// Exact accepted input formats.
    AcceptedFormats(#[serde(deserialize_with = "deserialize_collection_vec")] Vec<SourceFormatV1>),
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

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum EngineFactValueWireV1 {
    AcceptedFormats(
        #[serde(deserialize_with = "deserialize_collection_rows")] CappedSequence<SourceFormatV1>,
    ),
    AnimationAddressability(EngineAnimationAddressabilityV1),
    CoordinateBasis(EngineCoordinateBasisV1),
    LinearUnit(EngineLinearUnitV1),
    ConversionControl(EngineConversionControlV1),
    Boolean(bool),
    ImportHandling(EngineImportHandlingV1),
    TargetAddressability(EngineTargetAddressabilityV1),
    RootMotionAddressability(EngineRootMotionAddressabilityV1),
}

impl TryFrom<EngineFactValueWireV1> for EngineFactValueV1 {
    type Error = EngineContractError;

    fn try_from(wire: EngineFactValueWireV1) -> Result<Self, Self::Error> {
        Ok(match wire {
            EngineFactValueWireV1::AcceptedFormats(formats) => {
                if formats.overflowed {
                    return Err(EngineContractError::InvalidAcceptedInputs);
                }
                Self::AcceptedFormats(formats.values)
            }
            EngineFactValueWireV1::AnimationAddressability(value) => {
                Self::AnimationAddressability(value)
            }
            EngineFactValueWireV1::CoordinateBasis(value) => Self::CoordinateBasis(value),
            EngineFactValueWireV1::LinearUnit(value) => Self::LinearUnit(value),
            EngineFactValueWireV1::ConversionControl(value) => Self::ConversionControl(value),
            EngineFactValueWireV1::Boolean(value) => Self::Boolean(value),
            EngineFactValueWireV1::ImportHandling(value) => Self::ImportHandling(value),
            EngineFactValueWireV1::TargetAddressability(value) => Self::TargetAddressability(value),
            EngineFactValueWireV1::RootMotionAddressability(value) => {
                Self::RootMotionAddressability(value)
            }
        })
    }
}

impl<'de> Deserialize<'de> for EngineFactValueV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        EngineFactValueWireV1::deserialize(deserializer)?
            .try_into()
            .map_err(D::Error::custom)
    }
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnginePrimarySourceV1 {
    id: String,
    target_version: String,
    url: String,
    verified_on: String,
    supported_fact_ids: Vec<EngineFactIdV1>,
    supported_setting_ids: Vec<EngineSettingIdV1>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnginePrimarySourceWireV1 {
    id: String,
    target_version: String,
    url: String,
    verified_on: String,
    #[serde(deserialize_with = "deserialize_collection_rows")]
    supported_fact_ids: CappedSequence<EngineFactIdV1>,
    #[serde(deserialize_with = "deserialize_collection_rows")]
    supported_setting_ids: CappedSequence<EngineSettingIdV1>,
}

struct EnginePrimarySourceSeed<'a> {
    rows: &'a mut ProfileRows,
}

impl<'de> DeserializeSeed<'de> for EnginePrimarySourceSeed<'_> {
    type Value = EnginePrimarySourceWireV1;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Id,
            TargetVersion,
            Url,
            VerifiedOn,
            SupportedFactIds,
            SupportedSettingIds,
        }

        struct PrimarySourceVisitor<'a> {
            rows: &'a mut ProfileRows,
        }

        impl<'de> Visitor<'de> for PrimarySourceVisitor<'_> {
            type Value = EnginePrimarySourceWireV1;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an engine primary-source record")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut id = None;
                let mut target_version = None;
                let mut url = None;
                let mut verified_on = None;
                let mut supported_fact_ids = None;
                let mut supported_setting_ids = None;
                while let Some(field) = map.next_key()? {
                    match field {
                        Field::Id => set_once(&mut id, map.next_value()?, "id")?,
                        Field::TargetVersion => {
                            set_once(&mut target_version, map.next_value()?, "target_version")?
                        }
                        Field::Url => set_once(&mut url, map.next_value()?, "url")?,
                        Field::VerifiedOn => {
                            set_once(&mut verified_on, map.next_value()?, "verified_on")?
                        }
                        Field::SupportedFactIds => {
                            if supported_fact_ids.is_some() {
                                return Err(A::Error::duplicate_field("supported_fact_ids"));
                            }
                            supported_fact_ids =
                                Some(map.next_value_seed(BudgetedCappedSequenceSeed {
                                    budget: &mut self.rows.local,
                                    local_limit: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                                    element: PhantomData,
                                })?);
                        }
                        Field::SupportedSettingIds => {
                            if supported_setting_ids.is_some() {
                                return Err(A::Error::duplicate_field("supported_setting_ids"));
                            }
                            supported_setting_ids =
                                Some(map.next_value_seed(BudgetedCappedSequenceSeed {
                                    budget: &mut self.rows.local,
                                    local_limit: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                                    element: PhantomData,
                                })?);
                        }
                    }
                }
                Ok(EnginePrimarySourceWireV1 {
                    id: required(id, "id")?,
                    target_version: required(target_version, "target_version")?,
                    url: required(url, "url")?,
                    verified_on: required(verified_on, "verified_on")?,
                    supported_fact_ids: required(supported_fact_ids, "supported_fact_ids")?,
                    supported_setting_ids: required(
                        supported_setting_ids,
                        "supported_setting_ids",
                    )?,
                })
            }
        }

        deserializer.deserialize_struct(
            "EnginePrimarySourceV1",
            &[
                "id",
                "target_version",
                "url",
                "verified_on",
                "supported_fact_ids",
                "supported_setting_ids",
            ],
            PrimarySourceVisitor { rows: self.rows },
        )
    }
}

fn set_once<E, T>(slot: &mut Option<T>, value: T, field: &'static str) -> Result<(), E>
where
    E: serde::de::Error,
{
    if slot.replace(value).is_some() {
        return Err(E::duplicate_field(field));
    }
    Ok(())
}

fn required<E, T>(value: Option<T>, field: &'static str) -> Result<T, E>
where
    E: serde::de::Error,
{
    value.ok_or_else(|| E::missing_field(field))
}

impl EnginePrimarySourceV1 {
    fn from_wire(wire: EnginePrimarySourceWireV1) -> Result<Self, EngineContractError> {
        validate_required_text("primary_sources.id", &wire.id)?;
        validate_required_text("primary_sources.target_version", &wire.target_version)?;
        validate_required_text("primary_sources.url", &wire.url)?;
        validate_required_text("primary_sources.verified_on", &wire.verified_on)?;
        if wire.supported_fact_ids.overflowed {
            return Err(EngineContractError::TooManyRows {
                field: "primary_sources.supported_fact_ids",
                found: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
            });
        }
        if wire.supported_setting_ids.overflowed {
            return Err(EngineContractError::TooManyRows {
                field: "primary_sources.supported_setting_ids",
                found: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
            });
        }
        let source = Self {
            id: wire.id,
            target_version: wire.target_version,
            url: wire.url,
            verified_on: wire.verified_on,
            supported_fact_ids: wire.supported_fact_ids.values,
            supported_setting_ids: wire.supported_setting_ids.values,
        };
        source.validate(true)?;
        Ok(source)
    }
}

impl<'de> Deserialize<'de> for EnginePrimarySourceV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::from_wire(EnginePrimarySourceWireV1::deserialize(deserializer)?)
            .map_err(D::Error::custom)
    }
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

    pub(crate) fn provenance_rows(&self) -> usize {
        self.facts
            .len()
            .saturating_add(self.setting_descriptors.len())
            .saturating_add(self.primary_sources.len())
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

struct ResolvedEngineProfileWireV1 {
    schema: String,
    selection: EngineProfileSelectionV1,
    fact_bundle_urn: String,
    identity: InputIdentity,
    facts: CappedSequence<EngineProfileFactV1>,
    setting_descriptors: CappedSequence<EngineSettingDescriptorV1>,
    primary_sources: CappedSequence<EnginePrimarySourceWireV1>,
    aggregate_rows: RowBudget,
    provenance_rows_overflowed: bool,
}

enum ProfileTopLevelElement<T> {
    Value(T),
    Skipped,
}

struct ProfileTopLevelElementSeed<'a, T> {
    rows: &'a mut ProfileRows,
    element: PhantomData<fn() -> T>,
}

impl<'de, T> DeserializeSeed<'de> for ProfileTopLevelElementSeed<'_, T>
where
    T: Deserialize<'de>,
{
    type Value = ProfileTopLevelElement<T>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        if self.rows.admit_top_level() {
            T::deserialize(deserializer).map(ProfileTopLevelElement::Value)
        } else {
            IgnoredAny::deserialize(deserializer).map(|_| ProfileTopLevelElement::Skipped)
        }
    }
}

struct ProfileTopLevelSequenceSeed<'a, T> {
    rows: &'a mut ProfileRows,
    element: PhantomData<fn() -> T>,
}

impl<'de, T> DeserializeSeed<'de> for ProfileTopLevelSequenceSeed<'_, T>
where
    T: Deserialize<'de>,
{
    type Value = CappedSequence<T>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ProfileTopLevelSequenceVisitor<'a, T> {
            rows: &'a mut ProfileRows,
            element: PhantomData<fn() -> T>,
        }

        impl<'de, T> Visitor<'de> for ProfileTopLevelSequenceVisitor<'_, T>
        where
            T: Deserialize<'de>,
        {
            type Value = CappedSequence<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a bounded sequence of engine profile rows")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::with_capacity(
                    sequence
                        .size_hint()
                        .unwrap_or(0)
                        .min(ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS),
                );
                let mut seen = 0usize;
                while seen < ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS {
                    let Some(element) = sequence.next_element_seed(ProfileTopLevelElementSeed {
                        rows: self.rows,
                        element: PhantomData,
                    })?
                    else {
                        return Ok(CappedSequence {
                            values,
                            overflowed: false,
                        });
                    };
                    seen += 1;
                    match element {
                        ProfileTopLevelElement::Value(value) => values.push(value),
                        ProfileTopLevelElement::Skipped => {
                            let overflowed = consume_ignored_tail(
                                &mut sequence,
                                seen,
                                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                            )?;
                            return Ok(CappedSequence { values, overflowed });
                        }
                    }
                }
                let overflowed = consume_ignored_tail(
                    &mut sequence,
                    seen,
                    ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                )?;
                Ok(CappedSequence { values, overflowed })
            }
        }

        deserializer.deserialize_seq(ProfileTopLevelSequenceVisitor {
            rows: self.rows,
            element: PhantomData,
        })
    }
}

enum PrimarySourceElement {
    Value(EnginePrimarySourceWireV1),
    Skipped,
}

struct PrimarySourceElementSeed<'a> {
    rows: &'a mut ProfileRows,
}

impl<'de> DeserializeSeed<'de> for PrimarySourceElementSeed<'_> {
    type Value = PrimarySourceElement;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        if self.rows.admit_top_level() {
            EnginePrimarySourceSeed { rows: self.rows }
                .deserialize(deserializer)
                .map(PrimarySourceElement::Value)
        } else {
            IgnoredAny::deserialize(deserializer).map(|_| PrimarySourceElement::Skipped)
        }
    }
}

struct PrimarySourcesSeed<'a> {
    rows: &'a mut ProfileRows,
}

impl<'de> DeserializeSeed<'de> for PrimarySourcesSeed<'_> {
    type Value = CappedSequence<EnginePrimarySourceWireV1>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct PrimarySourcesVisitor<'a> {
            rows: &'a mut ProfileRows,
        }

        impl<'de> Visitor<'de> for PrimarySourcesVisitor<'_> {
            type Value = CappedSequence<EnginePrimarySourceWireV1>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a bounded sequence of engine primary sources")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::with_capacity(
                    sequence
                        .size_hint()
                        .unwrap_or(0)
                        .min(ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS),
                );
                let mut seen = 0usize;
                while seen < ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS {
                    let Some(element) =
                        sequence.next_element_seed(PrimarySourceElementSeed { rows: self.rows })?
                    else {
                        return Ok(CappedSequence {
                            values,
                            overflowed: false,
                        });
                    };
                    seen += 1;
                    match element {
                        PrimarySourceElement::Value(value) => values.push(value),
                        PrimarySourceElement::Skipped => {
                            let overflowed = consume_ignored_tail(
                                &mut sequence,
                                seen,
                                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                            )?;
                            return Ok(CappedSequence { values, overflowed });
                        }
                    }
                }
                let overflowed = consume_ignored_tail(
                    &mut sequence,
                    seen,
                    ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                )?;
                Ok(CappedSequence { values, overflowed })
            }
        }

        deserializer.deserialize_seq(PrimarySourcesVisitor { rows: self.rows })
    }
}

struct ResolvedEngineProfileWireSeed {
    provenance_limit: Option<usize>,
}

impl<'de> DeserializeSeed<'de> for ResolvedEngineProfileWireSeed {
    type Value = ResolvedEngineProfileWireV1;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Schema,
            Selection,
            FactBundleUrn,
            Identity,
            Facts,
            SettingDescriptors,
            PrimarySources,
        }

        struct ProfileVisitor {
            provenance_limit: Option<usize>,
        }

        impl<'de> Visitor<'de> for ProfileVisitor {
            type Value = ResolvedEngineProfileWireV1;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a resolved engine profile")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut rows = ProfileRows::new(self.provenance_limit);
                let mut schema = None;
                let mut selection = None;
                let mut fact_bundle_urn = None;
                let mut identity = None;
                let mut facts = None;
                let mut setting_descriptors = None;
                let mut primary_sources = None;
                while let Some(field) = map.next_key()? {
                    match field {
                        Field::Schema => set_once(&mut schema, map.next_value()?, "schema")?,
                        Field::Selection => {
                            set_once(&mut selection, map.next_value()?, "selection")?
                        }
                        Field::FactBundleUrn => {
                            set_once(&mut fact_bundle_urn, map.next_value()?, "fact_bundle_urn")?
                        }
                        Field::Identity => set_once(&mut identity, map.next_value()?, "identity")?,
                        Field::Facts => {
                            if facts.is_some() {
                                return Err(A::Error::duplicate_field("facts"));
                            }
                            facts = Some(map.next_value_seed(ProfileTopLevelSequenceSeed {
                                rows: &mut rows,
                                element: PhantomData,
                            })?);
                        }
                        Field::SettingDescriptors => {
                            if setting_descriptors.is_some() {
                                return Err(A::Error::duplicate_field("setting_descriptors"));
                            }
                            setting_descriptors =
                                Some(map.next_value_seed(ProfileTopLevelSequenceSeed {
                                    rows: &mut rows,
                                    element: PhantomData,
                                })?);
                        }
                        Field::PrimarySources => {
                            if primary_sources.is_some() {
                                return Err(A::Error::duplicate_field("primary_sources"));
                            }
                            primary_sources =
                                Some(map.next_value_seed(PrimarySourcesSeed { rows: &mut rows })?);
                        }
                    }
                }
                Ok(ResolvedEngineProfileWireV1 {
                    schema: required(schema, "schema")?,
                    selection: required(selection, "selection")?,
                    fact_bundle_urn: required(fact_bundle_urn, "fact_bundle_urn")?,
                    identity: required(identity, "identity")?,
                    facts: required(facts, "facts")?,
                    setting_descriptors: required(setting_descriptors, "setting_descriptors")?,
                    primary_sources: required(primary_sources, "primary_sources")?,
                    provenance_rows_overflowed: rows.provenance_overflowed(),
                    aggregate_rows: rows.local,
                })
            }
        }

        deserializer.deserialize_struct(
            "ResolvedEngineProfileV1",
            &[
                "schema",
                "selection",
                "fact_bundle_urn",
                "identity",
                "facts",
                "setting_descriptors",
                "primary_sources",
            ],
            ProfileVisitor {
                provenance_limit: self.provenance_limit,
            },
        )
    }
}

impl<'de> Deserialize<'de> for ResolvedEngineProfileWireV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        ResolvedEngineProfileWireSeed {
            provenance_limit: None,
        }
        .deserialize(deserializer)
    }
}

#[derive(Debug)]
pub(crate) enum EngineContractDecodeError {
    Shape(serde_json::Error),
    Semantic(EngineContractError),
}

impl ResolvedEngineProfileV1 {
    fn validate_wire_limits(wire: &ResolvedEngineProfileWireV1) -> Result<(), EngineContractError> {
        validate_schema("profile.schema", &wire.schema, ENGINE_PROFILE_FACTS_V1_ID)?;
        wire.selection.validate()?;
        validate_required_text("profile.fact_bundle_urn", &wire.fact_bundle_urn)?;
        for (field, overflowed) in [
            ("profile.facts", wire.facts.overflowed),
            (
                "profile.setting_descriptors",
                wire.setting_descriptors.overflowed,
            ),
            ("profile.primary_sources", wire.primary_sources.overflowed),
        ] {
            if overflowed {
                return Err(EngineContractError::TooManyRows {
                    field,
                    found: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                    max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                });
            }
        }
        for source in &wire.primary_sources.values {
            if source.supported_fact_ids.overflowed {
                return Err(EngineContractError::TooManyRows {
                    field: "primary_sources.supported_fact_ids",
                    found: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                    max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                });
            }
            if source.supported_setting_ids.overflowed {
                return Err(EngineContractError::TooManyRows {
                    field: "primary_sources.supported_setting_ids",
                    found: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                    max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                });
            }
        }
        if wire.aggregate_rows.overflowed() {
            return Err(EngineContractError::TooManyAggregateRows {
                found: wire.aggregate_rows.found(),
                max: ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS,
            });
        }
        Ok(())
    }

    fn from_wire(wire: ResolvedEngineProfileWireV1) -> Result<Self, EngineContractError> {
        Self::validate_wire_limits(&wire)?;
        let primary_sources = wire
            .primary_sources
            .values
            .into_iter()
            .map(EnginePrimarySourceV1::from_wire)
            .collect::<Result<Vec<_>, _>>()?;
        let profile = Self {
            schema: wire.schema,
            selection: wire.selection,
            fact_bundle_urn: wire.fact_bundle_urn,
            identity: wire.identity,
            facts: wire.facts.values,
            setting_descriptors: wire.setting_descriptors.values,
            primary_sources,
        };
        profile.validate()?;
        Ok(profile)
    }
}

#[cfg(test)]
pub(crate) fn decode_resolved_engine_profile_v1(
    raw: &str,
) -> Result<ResolvedEngineProfileV1, EngineContractDecodeError> {
    let wire = serde_json::from_str(raw).map_err(|source| {
        if source
            .to_string()
            .starts_with(&EngineContractError::InvalidAcceptedInputs.to_string())
        {
            EngineContractDecodeError::Semantic(EngineContractError::InvalidAcceptedInputs)
        } else {
            EngineContractDecodeError::Shape(source)
        }
    })?;
    ResolvedEngineProfileV1::from_wire(wire).map_err(EngineContractDecodeError::Semantic)
}

pub(crate) enum EngineProfileLimitedDecodeError {
    Contract(EngineContractDecodeError),
    ProvenanceRowsOverflow,
}

pub(crate) fn decode_resolved_engine_profile_v1_with_provenance_limit(
    raw: &str,
    provenance_limit: usize,
) -> Result<ResolvedEngineProfileV1, EngineProfileLimitedDecodeError> {
    let mut deserializer = serde_json::Deserializer::from_str(raw);
    let wire = ResolvedEngineProfileWireSeed {
        provenance_limit: Some(provenance_limit),
    }
    .deserialize(&mut deserializer)
    .map_err(|source| {
        if source
            .to_string()
            .starts_with(&EngineContractError::InvalidAcceptedInputs.to_string())
        {
            EngineProfileLimitedDecodeError::Contract(EngineContractDecodeError::Semantic(
                EngineContractError::InvalidAcceptedInputs,
            ))
        } else {
            EngineProfileLimitedDecodeError::Contract(EngineContractDecodeError::Shape(source))
        }
    })?;
    deserializer.end().map_err(|source| {
        EngineProfileLimitedDecodeError::Contract(EngineContractDecodeError::Shape(source))
    })?;
    ResolvedEngineProfileV1::validate_wire_limits(&wire).map_err(|source| {
        EngineProfileLimitedDecodeError::Contract(EngineContractDecodeError::Semantic(source))
    })?;
    if wire.provenance_rows_overflowed {
        return Err(EngineProfileLimitedDecodeError::ProvenanceRowsOverflow);
    }
    ResolvedEngineProfileV1::from_wire(wire).map_err(|source| {
        EngineProfileLimitedDecodeError::Contract(EngineContractDecodeError::Semantic(source))
    })
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EngineClipSettingsV1 {
    clip_name: String,
    settings: Vec<EngineSettingRowV1>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EngineClipSettingsWireV1 {
    clip_name: String,
    #[serde(deserialize_with = "deserialize_collection_rows")]
    settings: CappedSequence<EngineSettingRowV1>,
}

struct EngineClipSettingsSeed<'a> {
    rows: &'a mut SettingsRows,
}

enum SettingsElement<T> {
    Value(T),
    Skipped,
}

struct SettingsElementSeed<'a, T> {
    rows: &'a mut SettingsRows,
    element: PhantomData<fn() -> T>,
}

impl<'de, T> DeserializeSeed<'de> for SettingsElementSeed<'_, T>
where
    T: Deserialize<'de>,
{
    type Value = SettingsElement<T>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        if self.rows.admit_setting() {
            T::deserialize(deserializer).map(SettingsElement::Value)
        } else {
            IgnoredAny::deserialize(deserializer).map(|_| SettingsElement::Skipped)
        }
    }
}

struct SettingsSequenceSeed<'a, T> {
    rows: &'a mut SettingsRows,
    element: PhantomData<fn() -> T>,
}

impl<'de, T> DeserializeSeed<'de> for SettingsSequenceSeed<'_, T>
where
    T: Deserialize<'de>,
{
    type Value = CappedSequence<T>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SettingsSequenceVisitor<'a, T> {
            rows: &'a mut SettingsRows,
            element: PhantomData<fn() -> T>,
        }

        impl<'de, T> Visitor<'de> for SettingsSequenceVisitor<'_, T>
        where
            T: Deserialize<'de>,
        {
            type Value = CappedSequence<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a bounded sequence of engine setting rows")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::with_capacity(
                    sequence
                        .size_hint()
                        .unwrap_or(0)
                        .min(ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS),
                );
                let mut seen = 0usize;
                while seen < ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS {
                    let Some(element) = sequence.next_element_seed(SettingsElementSeed {
                        rows: self.rows,
                        element: PhantomData,
                    })?
                    else {
                        return Ok(CappedSequence {
                            values,
                            overflowed: false,
                        });
                    };
                    seen += 1;
                    match element {
                        SettingsElement::Value(value) => values.push(value),
                        SettingsElement::Skipped => {
                            let overflowed = consume_ignored_tail(
                                &mut sequence,
                                seen,
                                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                            )?;
                            return Ok(CappedSequence { values, overflowed });
                        }
                    }
                }
                let overflowed = consume_ignored_tail(
                    &mut sequence,
                    seen,
                    ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                )?;
                Ok(CappedSequence { values, overflowed })
            }
        }

        deserializer.deserialize_seq(SettingsSequenceVisitor {
            rows: self.rows,
            element: PhantomData,
        })
    }
}

impl<'de> DeserializeSeed<'de> for EngineClipSettingsSeed<'_> {
    type Value = EngineClipSettingsWireV1;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            ClipName,
            Settings,
        }

        struct ClipSettingsVisitor<'a> {
            rows: &'a mut SettingsRows,
        }

        impl<'de> Visitor<'de> for ClipSettingsVisitor<'_> {
            type Value = EngineClipSettingsWireV1;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an engine clip-settings record")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut clip_name = None;
                let mut settings = None;
                while let Some(field) = map.next_key()? {
                    match field {
                        Field::ClipName => {
                            set_once(&mut clip_name, map.next_value()?, "clip_name")?
                        }
                        Field::Settings => {
                            if settings.is_some() {
                                return Err(A::Error::duplicate_field("settings"));
                            }
                            settings = Some(map.next_value_seed(SettingsSequenceSeed {
                                rows: self.rows,
                                element: PhantomData,
                            })?);
                        }
                    }
                }
                Ok(EngineClipSettingsWireV1 {
                    clip_name: required(clip_name, "clip_name")?,
                    settings: required(settings, "settings")?,
                })
            }
        }

        deserializer.deserialize_struct(
            "EngineClipSettingsV1",
            &["clip_name", "settings"],
            ClipSettingsVisitor { rows: self.rows },
        )
    }
}

impl EngineClipSettingsV1 {
    fn from_wire(wire: EngineClipSettingsWireV1) -> Result<Self, EngineContractError> {
        validate_text("settings.clips.clip_name", &wire.clip_name)?;
        if wire.settings.overflowed {
            return Err(EngineContractError::TooManyRows {
                field: "settings.clips.settings",
                found: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
            });
        }
        let clip = Self {
            clip_name: wire.clip_name,
            settings: wire.settings.values,
        };
        clip.validate(true)?;
        Ok(clip)
    }
}

impl<'de> Deserialize<'de> for EngineClipSettingsV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::from_wire(EngineClipSettingsWireV1::deserialize(deserializer)?)
            .map_err(D::Error::custom)
    }
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

struct ResolvedEngineSettingsWireV1 {
    schema: String,
    identity: InputIdentity,
    document_settings: CappedSequence<EngineSettingRowV1>,
    clips: CappedSequence<EngineClipSettingsWireV1>,
    aggregate_rows: RowBudget,
    provenance_rows_overflowed: bool,
}

enum ClipSettingsElement {
    Value(EngineClipSettingsWireV1),
    Skipped,
}

struct ClipSettingsElementSeed<'a> {
    rows: &'a mut SettingsRows,
}

impl<'de> DeserializeSeed<'de> for ClipSettingsElementSeed<'_> {
    type Value = ClipSettingsElement;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        if self.rows.admit_clip() {
            EngineClipSettingsSeed { rows: self.rows }
                .deserialize(deserializer)
                .map(ClipSettingsElement::Value)
        } else {
            IgnoredAny::deserialize(deserializer).map(|_| ClipSettingsElement::Skipped)
        }
    }
}

struct ClipSettingsSequenceSeed<'a> {
    rows: &'a mut SettingsRows,
}

impl<'de> DeserializeSeed<'de> for ClipSettingsSequenceSeed<'_> {
    type Value = CappedSequence<EngineClipSettingsWireV1>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ClipSettingsSequenceVisitor<'a> {
            rows: &'a mut SettingsRows,
        }

        impl<'de> Visitor<'de> for ClipSettingsSequenceVisitor<'_> {
            type Value = CappedSequence<EngineClipSettingsWireV1>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a bounded sequence of engine clip settings")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = Vec::with_capacity(
                    sequence
                        .size_hint()
                        .unwrap_or(0)
                        .min(ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS),
                );
                let mut seen = 0usize;
                while seen < ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS {
                    let Some(element) =
                        sequence.next_element_seed(ClipSettingsElementSeed { rows: self.rows })?
                    else {
                        return Ok(CappedSequence {
                            values,
                            overflowed: false,
                        });
                    };
                    seen += 1;
                    match element {
                        ClipSettingsElement::Value(value) => values.push(value),
                        ClipSettingsElement::Skipped => {
                            let overflowed = consume_ignored_tail(
                                &mut sequence,
                                seen,
                                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                            )?;
                            return Ok(CappedSequence { values, overflowed });
                        }
                    }
                }
                let overflowed = consume_ignored_tail(
                    &mut sequence,
                    seen,
                    ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                )?;
                Ok(CappedSequence { values, overflowed })
            }
        }

        deserializer.deserialize_seq(ClipSettingsSequenceVisitor { rows: self.rows })
    }
}

struct ResolvedEngineSettingsWireSeed {
    provenance_limit: Option<usize>,
}

impl<'de> DeserializeSeed<'de> for ResolvedEngineSettingsWireSeed {
    type Value = ResolvedEngineSettingsWireV1;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Schema,
            Identity,
            DocumentSettings,
            Clips,
        }

        struct SettingsVisitor {
            provenance_limit: Option<usize>,
        }

        impl<'de> Visitor<'de> for SettingsVisitor {
            type Value = ResolvedEngineSettingsWireV1;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("resolved engine settings")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut rows = SettingsRows::new(self.provenance_limit);
                let mut schema = None;
                let mut identity = None;
                let mut document_settings = None;
                let mut clips = None;
                while let Some(field) = map.next_key()? {
                    match field {
                        Field::Schema => set_once(&mut schema, map.next_value()?, "schema")?,
                        Field::Identity => set_once(&mut identity, map.next_value()?, "identity")?,
                        Field::DocumentSettings => {
                            if document_settings.is_some() {
                                return Err(A::Error::duplicate_field("document_settings"));
                            }
                            document_settings =
                                Some(map.next_value_seed(SettingsSequenceSeed {
                                    rows: &mut rows,
                                    element: PhantomData,
                                })?);
                        }
                        Field::Clips => {
                            if clips.is_some() {
                                return Err(A::Error::duplicate_field("clips"));
                            }
                            clips =
                                Some(map.next_value_seed(ClipSettingsSequenceSeed {
                                    rows: &mut rows,
                                })?);
                        }
                    }
                }
                Ok(ResolvedEngineSettingsWireV1 {
                    schema: required(schema, "schema")?,
                    identity: required(identity, "identity")?,
                    document_settings: required(document_settings, "document_settings")?,
                    clips: required(clips, "clips")?,
                    provenance_rows_overflowed: rows.provenance_overflowed(),
                    aggregate_rows: rows.local,
                })
            }
        }

        deserializer.deserialize_struct(
            "ResolvedEngineSettingsV1",
            &["schema", "identity", "document_settings", "clips"],
            SettingsVisitor {
                provenance_limit: self.provenance_limit,
            },
        )
    }
}

impl<'de> Deserialize<'de> for ResolvedEngineSettingsWireV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        ResolvedEngineSettingsWireSeed {
            provenance_limit: None,
        }
        .deserialize(deserializer)
    }
}

impl ResolvedEngineSettingsV1 {
    fn validate_wire_limits(
        wire: &ResolvedEngineSettingsWireV1,
    ) -> Result<(), EngineContractError> {
        validate_schema(
            "settings.schema",
            &wire.schema,
            RESOLVED_ENGINE_SETTINGS_V1_ID,
        )?;
        for (field, overflowed) in [
            (
                "settings.document_settings",
                wire.document_settings.overflowed,
            ),
            ("settings.clips", wire.clips.overflowed),
        ] {
            if overflowed {
                return Err(EngineContractError::TooManyRows {
                    field,
                    found: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                    max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                });
            }
        }
        for clip in &wire.clips.values {
            if clip.settings.overflowed {
                return Err(EngineContractError::TooManyRows {
                    field: "settings.clips.settings",
                    found: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                    max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                });
            }
        }
        if wire.aggregate_rows.overflowed() {
            return Err(EngineContractError::TooManyAggregateRows {
                found: wire.aggregate_rows.found(),
                max: ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS,
            });
        }
        Ok(())
    }

    fn from_wire(wire: ResolvedEngineSettingsWireV1) -> Result<Self, EngineContractError> {
        Self::validate_wire_limits(&wire)?;
        let clips = wire
            .clips
            .values
            .into_iter()
            .map(EngineClipSettingsV1::from_wire)
            .collect::<Result<Vec<_>, _>>()?;
        let settings = Self {
            schema: wire.schema,
            identity: wire.identity,
            document_settings: wire.document_settings.values,
            clips,
        };
        settings.validate_structure(true)?;
        Ok(settings)
    }
}

pub(crate) enum EngineSettingsLimitedDecodeError {
    Contract(EngineContractDecodeError),
    ProvenanceRowsOverflow,
}

pub(crate) fn decode_resolved_engine_settings_v1_with_provenance_limit(
    raw: &str,
    provenance_limit: usize,
) -> Result<ResolvedEngineSettingsV1, EngineSettingsLimitedDecodeError> {
    let mut deserializer = serde_json::Deserializer::from_str(raw);
    let wire = ResolvedEngineSettingsWireSeed {
        provenance_limit: Some(provenance_limit),
    }
    .deserialize(&mut deserializer)
    .map_err(|source| {
        EngineSettingsLimitedDecodeError::Contract(EngineContractDecodeError::Shape(source))
    })?;
    deserializer.end().map_err(|source| {
        EngineSettingsLimitedDecodeError::Contract(EngineContractDecodeError::Shape(source))
    })?;
    ResolvedEngineSettingsV1::validate_wire_limits(&wire).map_err(|source| {
        EngineSettingsLimitedDecodeError::Contract(EngineContractDecodeError::Semantic(source))
    })?;
    if wire.provenance_rows_overflowed {
        return Err(EngineSettingsLimitedDecodeError::ProvenanceRowsOverflow);
    }
    ResolvedEngineSettingsV1::from_wire(wire).map_err(|source| {
        EngineSettingsLimitedDecodeError::Contract(EngineContractDecodeError::Semantic(source))
    })
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

/// Complete or bounded-partial coverage of actual clip settings in V2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedEngineSettingsCoverageStateV2 {
    /// Every actual clip has a retained materialized settings row.
    Complete,
    /// Only the bounded canonical prefix was retained.
    Partial,
}

/// Stable reason for bounded-partial V2 settings coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolvedEngineSettingsCoverageReasonV2 {
    /// The actual source inventory exceeded the 4,096 retained-row limit.
    ActualClipRowsExceeded,
}

/// Explicit actual-clip coverage carried by resolved-engine-settings V2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedEngineSettingsCoverageV2 {
    state: ResolvedEngineSettingsCoverageStateV2,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<ResolvedEngineSettingsCoverageReasonV2>,
}

impl ResolvedEngineSettingsCoverageV2 {
    /// Construct complete coverage.
    pub const fn complete() -> Self {
        Self {
            state: ResolvedEngineSettingsCoverageStateV2::Complete,
            reason: None,
        }
    }

    /// Construct N+1 bounded partial coverage.
    pub const fn actual_clip_rows_exceeded() -> Self {
        Self {
            state: ResolvedEngineSettingsCoverageStateV2::Partial,
            reason: Some(ResolvedEngineSettingsCoverageReasonV2::ActualClipRowsExceeded),
        }
    }

    /// Coverage state.
    pub const fn state(&self) -> ResolvedEngineSettingsCoverageStateV2 {
        self.state
    }

    /// Partial-coverage reason, when applicable.
    pub const fn reason(&self) -> Option<ResolvedEngineSettingsCoverageReasonV2> {
        self.reason
    }
}

/// Bounded work accounting carried by resolved-engine-settings V2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedEngineSettingsWorkV2 {
    actual_clip_rows_inspected: usize,
    materialized_clip_rows: usize,
    retained_clip_rows: usize,
}

impl ResolvedEngineSettingsWorkV2 {
    /// Construct bounded work counters.
    pub const fn new(
        actual_clip_rows_inspected: usize,
        materialized_clip_rows: usize,
        retained_clip_rows: usize,
    ) -> Self {
        Self {
            actual_clip_rows_inspected,
            materialized_clip_rows,
            retained_clip_rows,
        }
    }

    /// Actual source rows inspected, capped at 4,097.
    pub const fn actual_clip_rows_inspected(&self) -> usize {
        self.actual_clip_rows_inspected
    }

    /// Rows whose settings were materialized.
    pub const fn materialized_clip_rows(&self) -> usize {
        self.materialized_clip_rows
    }

    /// Rows retained after canonicalization.
    pub const fn retained_clip_rows(&self) -> usize {
        self.retained_clip_rows
    }
}

/// Fully materialized bounded-prefix V2 engine settings.
///
/// V2 preserves the ordinary V1 row spelling while committing to coverage and
/// bounded work in a separate canonical identity. Thus an exact 4,096-row
/// input cannot collide with a larger input sharing that prefix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedEngineSettingsV2 {
    schema: String,
    identity: InputIdentity,
    document_settings: Vec<EngineSettingRowV1>,
    clips: Vec<EngineClipSettingsV1>,
    clip_coverage: ResolvedEngineSettingsCoverageV2,
    work: ResolvedEngineSettingsWorkV2,
}

struct ResolvedEngineSettingsWireV2 {
    schema: String,
    identity: InputIdentity,
    document_settings: CappedSequence<EngineSettingRowV1>,
    clips: CappedSequence<EngineClipSettingsWireV1>,
    clip_coverage: ResolvedEngineSettingsCoverageV2,
    work: ResolvedEngineSettingsWorkV2,
    aggregate_rows: RowBudget,
    provenance_rows_overflowed: bool,
}

struct ResolvedEngineSettingsWireSeedV2 {
    provenance_limit: Option<usize>,
}

impl<'de> DeserializeSeed<'de> for ResolvedEngineSettingsWireSeedV2 {
    type Value = ResolvedEngineSettingsWireV2;
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Schema,
            Identity,
            DocumentSettings,
            Clips,
            ClipCoverage,
            Work,
        }
        struct VisitorV2 {
            provenance_limit: Option<usize>,
        }
        impl<'de> Visitor<'de> for VisitorV2 {
            type Value = ResolvedEngineSettingsWireV2;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("resolved engine settings V2")
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut rows = SettingsRows::new(self.provenance_limit);
                let mut schema = None;
                let mut identity = None;
                let mut document_settings = None;
                let mut clips = None;
                let mut clip_coverage = None;
                let mut work = None;
                while let Some(field) = map.next_key()? {
                    match field {
                        Field::Schema => set_once(&mut schema, map.next_value()?, "schema")?,
                        Field::Identity => set_once(&mut identity, map.next_value()?, "identity")?,
                        Field::DocumentSettings => {
                            if document_settings.is_some() {
                                return Err(A::Error::duplicate_field("document_settings"));
                            }
                            document_settings =
                                Some(map.next_value_seed(SettingsSequenceSeed {
                                    rows: &mut rows,
                                    element: PhantomData,
                                })?);
                        }
                        Field::Clips => {
                            if clips.is_some() {
                                return Err(A::Error::duplicate_field("clips"));
                            }
                            clips =
                                Some(map.next_value_seed(ClipSettingsSequenceSeed {
                                    rows: &mut rows,
                                })?);
                        }
                        Field::ClipCoverage => {
                            set_once(&mut clip_coverage, map.next_value()?, "clip_coverage")?
                        }
                        Field::Work => set_once(&mut work, map.next_value()?, "work")?,
                    }
                }
                Ok(ResolvedEngineSettingsWireV2 {
                    schema: required(schema, "schema")?,
                    identity: required(identity, "identity")?,
                    document_settings: required(document_settings, "document_settings")?,
                    clips: required(clips, "clips")?,
                    clip_coverage: required(clip_coverage, "clip_coverage")?,
                    work: required(work, "work")?,
                    provenance_rows_overflowed: rows.provenance_overflowed(),
                    aggregate_rows: rows.local,
                })
            }
        }
        deserializer.deserialize_struct(
            "ResolvedEngineSettingsV2",
            &[
                "schema",
                "identity",
                "document_settings",
                "clips",
                "clip_coverage",
                "work",
            ],
            VisitorV2 {
                provenance_limit: self.provenance_limit,
            },
        )
    }
}

impl<'de> Deserialize<'de> for ResolvedEngineSettingsWireV2 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        ResolvedEngineSettingsWireSeedV2 {
            provenance_limit: None,
        }
        .deserialize(deserializer)
    }
}

impl ResolvedEngineSettingsV2 {
    /// Construct V2 settings from a fully materialized retained prefix.
    pub fn new(
        profile: &ResolvedEngineProfileV1,
        document_settings: Vec<EngineSettingRowV1>,
        clips: Vec<EngineClipSettingsV1>,
        clip_coverage: ResolvedEngineSettingsCoverageV2,
        work: ResolvedEngineSettingsWorkV2,
    ) -> Result<Self, EngineContractError> {
        let prefix = ResolvedEngineSettingsV1::new(profile, document_settings, clips)?;
        let mut settings = Self {
            schema: RESOLVED_ENGINE_SETTINGS_V2_ID.to_owned(),
            identity: InputIdentity::from_bytes(&[]),
            document_settings: prefix.document_settings,
            clips: prefix.clips,
            clip_coverage,
            work,
        };
        settings.validate_prefix_against(profile)?;
        settings.validate_coverage_work()?;
        settings.identity = settings.computed_identity(profile);
        Ok(settings)
    }

    /// Contract id carried in `schema`.
    pub fn contract_id(&self) -> &str {
        &self.schema
    }

    /// Identity committing to prefix, coverage, and work.
    pub const fn settings_identity(&self) -> &InputIdentity {
        &self.identity
    }

    /// Stable-id-ordered document settings.
    pub fn document_settings(&self) -> &[EngineSettingRowV1] {
        &self.document_settings
    }

    /// Canonically ordered retained clip rows.
    pub fn clips(&self) -> &[EngineClipSettingsV1] {
        &self.clips
    }

    /// Look up one retained document setting.
    pub fn document_setting(&self, id: EngineSettingIdV1) -> Option<&EngineSettingValueV1> {
        self.document_settings
            .iter()
            .find(|row| row.id == id)
            .map(|row| &row.value)
    }

    /// Look up one retained clip row by source ordinal and exact name.
    pub fn clip_row(&self, ordinal: usize, clip_name: &str) -> Option<&EngineClipSettingsV1> {
        self.clips
            .get(ordinal)
            .filter(|row| row.clip_name == clip_name)
    }

    /// Aggregate retained text in the shared settings prefix.
    pub(crate) fn retained_text_bytes(&self) -> Result<usize, EngineContractError> {
        self.v1_prefix().retained_text_bytes()
    }

    /// Explicit actual-clip coverage.
    pub const fn clip_coverage(&self) -> &ResolvedEngineSettingsCoverageV2 {
        &self.clip_coverage
    }

    /// Bounded resolution work.
    pub const fn work(&self) -> &ResolvedEngineSettingsWorkV2 {
        &self.work
    }

    /// Validate settings, coverage/work, and the V2 identity against profile facts.
    pub fn validate_against(
        &self,
        profile: &ResolvedEngineProfileV1,
    ) -> Result<(), EngineContractError> {
        self.validate_prefix_against(profile)?;
        self.validate_coverage_work()?;
        if self.identity != self.computed_identity(profile) {
            return Err(EngineContractError::IdentityMismatch {
                contract: RESOLVED_ENGINE_SETTINGS_V2_ID,
            });
        }
        Ok(())
    }

    fn validate_prefix_against(
        &self,
        profile: &ResolvedEngineProfileV1,
    ) -> Result<(), EngineContractError> {
        if self.schema != RESOLVED_ENGINE_SETTINGS_V2_ID {
            return Err(EngineContractError::InvalidSchema {
                field: "settings.schema",
                expected: RESOLVED_ENGINE_SETTINGS_V2_ID,
                found: self.schema.clone(),
            });
        }
        let prefix = self.v1_prefix();
        prefix.validate_structure(true)?;
        prefix.validate_materialization(profile, false)
    }

    fn validate_coverage_work(&self) -> Result<(), EngineContractError> {
        let retained = self.clips.len();
        let complete = matches!(
            (self.clip_coverage.state, self.clip_coverage.reason),
            (ResolvedEngineSettingsCoverageStateV2::Complete, None)
        );
        let partial = matches!(
            (self.clip_coverage.state, self.clip_coverage.reason),
            (
                ResolvedEngineSettingsCoverageStateV2::Partial,
                Some(ResolvedEngineSettingsCoverageReasonV2::ActualClipRowsExceeded)
            )
        );
        let expected_inspected = if partial {
            ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1
        } else {
            retained
        };
        if !(complete || partial)
            || self.work.actual_clip_rows_inspected != expected_inspected
            || self.work.materialized_clip_rows != retained
            || self.work.retained_clip_rows != retained
            || (partial && retained != ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS)
        {
            return Err(EngineContractError::InvalidV2CoverageWork);
        }
        Ok(())
    }

    fn v1_prefix(&self) -> ResolvedEngineSettingsV1 {
        ResolvedEngineSettingsV1 {
            schema: RESOLVED_ENGINE_SETTINGS_V1_ID.to_owned(),
            identity: InputIdentity::from_bytes(&[]),
            document_settings: self.document_settings.clone(),
            clips: self.clips.clone(),
        }
    }

    /// Construct an internal V1-shaped prefix solely to reuse established
    /// profile/raw-source cross-link validation. It is never serialized or
    /// returned from a V2 public API.
    pub(crate) fn validation_only_prefix(
        &self,
        profile: &ResolvedEngineProfileV1,
    ) -> Result<ResolvedEngineSettingsV1, EngineContractError> {
        ResolvedEngineSettingsV1::new(profile, self.document_settings.clone(), self.clips.clone())
    }

    fn computed_identity(&self, profile: &ResolvedEngineProfileV1) -> InputIdentity {
        let mut encoder = CanonicalEncoder::new("animsmith-engine-settings-v2");
        self.v1_prefix().encode_preimage(profile, &mut encoder);
        encoder.field("clip_coverage.state");
        encoder.token(match self.clip_coverage.state {
            ResolvedEngineSettingsCoverageStateV2::Complete => "complete",
            ResolvedEngineSettingsCoverageStateV2::Partial => "partial",
        });
        encoder.field("clip_coverage.reason");
        encoder.token(match self.clip_coverage.reason {
            None => "none",
            Some(ResolvedEngineSettingsCoverageReasonV2::ActualClipRowsExceeded) => {
                "actual_clip_rows_exceeded"
            }
        });
        for (field, value) in [
            (
                "actual_clip_rows_inspected",
                self.work.actual_clip_rows_inspected,
            ),
            ("materialized_clip_rows", self.work.materialized_clip_rows),
            ("retained_clip_rows", self.work.retained_clip_rows),
        ] {
            encoder.field(field);
            encoder.count(value);
        }
        encoder.identity()
    }
}

impl<'de> Deserialize<'de> for ResolvedEngineSettingsV2 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ResolvedEngineSettingsWireV2::deserialize(deserializer)?;
        if wire.document_settings.overflowed || wire.clips.overflowed {
            return Err(D::Error::custom(
                "resolved-engine-settings V2 collection exceeds 4096 rows",
            ));
        }
        if wire.aggregate_rows.overflowed() {
            return Err(D::Error::custom(
                EngineContractError::TooManyAggregateRows {
                    found: wire.aggregate_rows.found(),
                    max: ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS,
                },
            ));
        }
        let settings = Self {
            schema: wire.schema,
            identity: wire.identity,
            document_settings: wire.document_settings.values,
            clips: wire
                .clips
                .values
                .into_iter()
                .map(EngineClipSettingsV1::from_wire)
                .collect::<Result<_, _>>()
                .map_err(D::Error::custom)?,
            clip_coverage: wire.clip_coverage,
            work: wire.work,
        };
        if settings.schema != RESOLVED_ENGINE_SETTINGS_V2_ID {
            return Err(D::Error::custom(
                "invalid resolved-engine-settings V2 schema",
            ));
        }
        settings
            .v1_prefix()
            .validate_structure(true)
            .map_err(D::Error::custom)?;
        settings
            .validate_coverage_work()
            .map_err(D::Error::custom)?;
        Ok(settings)
    }
}

/// Decode V2 settings under the caller-owned remaining provenance-row budget.
/// The seed admits only rows that fit the shared budget and consumes the first
/// excess row as a sentinel, so no partial settings prefix is returned.
pub(crate) fn decode_resolved_engine_settings_v2_with_provenance_limit(
    raw: &str,
    provenance_limit: usize,
) -> Result<ResolvedEngineSettingsV2, EngineSettingsLimitedDecodeError> {
    let mut deserializer = serde_json::Deserializer::from_str(raw);
    let wire = ResolvedEngineSettingsWireSeedV2 {
        provenance_limit: Some(provenance_limit),
    }
    .deserialize(&mut deserializer)
    .map_err(|source| {
        EngineSettingsLimitedDecodeError::Contract(EngineContractDecodeError::Shape(source))
    })?;
    deserializer.end().map_err(|source| {
        EngineSettingsLimitedDecodeError::Contract(EngineContractDecodeError::Shape(source))
    })?;
    if wire.provenance_rows_overflowed {
        return Err(EngineSettingsLimitedDecodeError::ProvenanceRowsOverflow);
    }
    let settings = ResolvedEngineSettingsV2 {
        schema: wire.schema,
        identity: wire.identity,
        document_settings: wire.document_settings.values,
        clips: wire
            .clips
            .values
            .into_iter()
            .map(EngineClipSettingsV1::from_wire)
            .collect::<Result<_, _>>()
            .map_err(|source| {
                EngineSettingsLimitedDecodeError::Contract(EngineContractDecodeError::Semantic(
                    source,
                ))
            })?,
        clip_coverage: wire.clip_coverage,
        work: wire.work,
    };
    settings
        .v1_prefix()
        .validate_structure(true)
        .map_err(|source| {
            EngineSettingsLimitedDecodeError::Contract(EngineContractDecodeError::Semantic(source))
        })?;
    settings.validate_coverage_work().map_err(|source| {
        EngineSettingsLimitedDecodeError::Contract(EngineContractDecodeError::Semantic(source))
    })?;
    Ok(settings)
}

/// Positive reduced rational used where binary floating point would change an
/// exact importer contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "ReducedRatioWireV1")]
pub struct ReducedRatioV1 {
    numerator: u64,
    denominator: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReducedRatioWireV1 {
    numerator: u64,
    denominator: u64,
}

impl ReducedRatioV1 {
    /// Construct a positive ratio in lowest terms.
    pub fn new(numerator: u64, denominator: u64) -> Result<Self, EngineContractError> {
        let ratio = Self {
            numerator,
            denominator,
        };
        ratio.validate()?;
        Ok(ratio)
    }

    /// Reduced numerator.
    pub const fn numerator(self) -> u64 {
        self.numerator
    }

    /// Reduced nonzero denominator.
    pub const fn denominator(self) -> u64 {
        self.denominator
    }

    fn validate(self) -> Result<(), EngineContractError> {
        if self.numerator == 0
            || self.denominator == 0
            || greatest_common_divisor(self.numerator, self.denominator) != 1
        {
            return Err(EngineContractError::InvalidReducedRatio {
                numerator: self.numerator,
                denominator: self.denominator,
            });
        }
        Ok(())
    }
}

impl TryFrom<ReducedRatioWireV1> for ReducedRatioV1 {
    type Error = EngineContractError;

    fn try_from(wire: ReducedRatioWireV1) -> Result<Self, Self::Error> {
        Self::new(wire.numerator, wire.denominator)
    }
}

const fn greatest_common_divisor(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

/// Closed V2 fact vocabulary shared by prediction and advice profiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineFactIdV2 {
    /// Accepted source containers.
    AcceptedInputs,
    /// Target world-length convention, without claiming application enforcement.
    TargetLinearUnit,
    /// Exact source numeric-unit mapping.
    SourceToTargetUnitMapping,
    /// Whether numeric physical dimensions survive importer conversion.
    PhysicalDimensionsPreserved,
    /// Importer scale-conversion behavior.
    ImporterScaleConversion,
    /// Whether the engine enforces an application-wide world-unit policy.
    ApplicationWorldUnitPolicy,
    /// Effective static/default-rest transform behavior.
    ResultingTransformScale,
    /// Root-motion source/addressability semantics.
    RootMotionAddressability,
    /// Import handling for source animation/channel subjects.
    SourceImportDisposition,
    /// Exact import-setting projection semantics.
    ImportSettingProjection,
}

impl EngineFactIdV2 {
    /// Stable wire spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AcceptedInputs => "accepted_inputs",
            Self::TargetLinearUnit => "target_linear_unit",
            Self::SourceToTargetUnitMapping => "source_to_target_unit_mapping",
            Self::PhysicalDimensionsPreserved => "physical_dimensions_preserved",
            Self::ImporterScaleConversion => "importer_scale_conversion",
            Self::ApplicationWorldUnitPolicy => "application_world_unit_policy",
            Self::ResultingTransformScale => "resulting_transform_scale",
            Self::RootMotionAddressability => "root_motion_addressability",
            Self::SourceImportDisposition => "source_import_disposition",
            Self::ImportSettingProjection => "import_setting_projection",
        }
    }
}

const ALL_FACT_IDS_V2: [EngineFactIdV2; 10] = [
    EngineFactIdV2::AcceptedInputs,
    EngineFactIdV2::ApplicationWorldUnitPolicy,
    EngineFactIdV2::ImportSettingProjection,
    EngineFactIdV2::ImporterScaleConversion,
    EngineFactIdV2::PhysicalDimensionsPreserved,
    EngineFactIdV2::ResultingTransformScale,
    EngineFactIdV2::RootMotionAddressability,
    EngineFactIdV2::SourceImportDisposition,
    EngineFactIdV2::SourceToTargetUnitMapping,
    EngineFactIdV2::TargetLinearUnit,
];

/// Closed unit vocabulary for exact importer prediction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineLinearUnitV2 {
    /// glTF metre-per-unit semantics.
    Metre,
    /// Centimetre-authored numeric units.
    Centimetre,
    /// One engine world-space length unit, without a physical-unit guarantee.
    EngineWorldLengthUnit,
}

/// Closed V2 fact value vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineFactValueV2 {
    /// Exact accepted formats.
    AcceptedFormats(Vec<SourceFormatV1>),
    /// A unit convention.
    LinearUnit(EngineLinearUnitV2),
    /// Exact target units per source unit.
    UnitRatio(ReducedRatioV1),
    /// Boolean fact.
    Boolean(bool),
    /// A closed semantic token whose vocabulary is owned by the fact id.
    Token(String),
    /// Root-motion addressability from the historical closed vocabulary.
    RootMotionAddressability(EngineRootMotionAddressabilityV1),
}

/// Evidence state of one V2 profile fact.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineFactStateV2 {
    /// Supported exact value.
    Known(EngineFactValueV2),
    /// Primary evidence does not establish a value.
    Unknown,
    /// The domain genuinely does not apply.
    NotApplicable,
}

/// One immutable V2 fact row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineProfileFactV2 {
    id: EngineFactIdV2,
    state: EngineFactStateV2,
}

impl EngineProfileFactV2 {
    /// Construct one fact row.
    pub const fn new(id: EngineFactIdV2, state: EngineFactStateV2) -> Self {
        Self { id, state }
    }

    /// Stable fact id.
    pub const fn id(&self) -> EngineFactIdV2 {
        self.id
    }

    /// Explicit evidence state.
    pub const fn state(&self) -> &EngineFactStateV2 {
        &self.state
    }
}

/// Closed setting ids required by the coordinated V2 profile migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineSettingIdV2 {
    /// Historical unit conversion toggle.
    ConvertUnits,
    /// Historical axis conversion toggle.
    BakeAxisConversion,
    /// Exact root-motion source path.
    RootMotionSource,
    /// Yaw bake/extract policy.
    RootRotation,
    /// Vertical bake/extract policy.
    RootPositionY,
    /// Horizontal bake/extract policy.
    RootPositionXz,
    /// Unity animation type.
    AnimationType,
    /// Unity avatar construction policy.
    AvatarSetup,
    /// Unity animation import gate.
    ImportAnimation,
    /// Bevy scene-entity coordinate rotation.
    RotateSceneEntity,
    /// Bevy mesh coordinate rotation.
    RotateMeshes,
    /// Bevy mesh-name load filter.
    LoadMeshes,
    /// Exact Bevy extension-handler environment.
    ExtensionHandlerEnvironment,
    /// Compile-time Bevy animation feature.
    BevyAnimationFeature,
    /// Per-load Bevy animation gate.
    LoadAnimations,
    /// Godot animation bake frequency.
    AnimationFps,
    /// Godot animation trimming toggle.
    AnimationTrimming,
    /// Unreal sample-rate policy.
    SampleRate,
}

impl EngineSettingIdV2 {
    /// Stable wire spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConvertUnits => "convert_units",
            Self::BakeAxisConversion => "bake_axis_conversion",
            Self::RootMotionSource => "root_motion_source",
            Self::RootRotation => "root_rotation",
            Self::RootPositionY => "root_position_y",
            Self::RootPositionXz => "root_position_xz",
            Self::AnimationType => "animation_type",
            Self::AvatarSetup => "avatar_setup",
            Self::ImportAnimation => "import_animation",
            Self::RotateSceneEntity => "rotate_scene_entity",
            Self::RotateMeshes => "rotate_meshes",
            Self::LoadMeshes => "load_meshes",
            Self::ExtensionHandlerEnvironment => "extension_handler_environment",
            Self::BevyAnimationFeature => "bevy_animation_feature",
            Self::LoadAnimations => "load_animations",
            Self::AnimationFps => "animation_fps",
            Self::AnimationTrimming => "animation_trimming",
            Self::SampleRate => "sample_rate",
        }
    }
}

/// Closed V2 setting value domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineSettingDomainV2 {
    /// Boolean.
    Boolean,
    /// Positive bounded integer.
    PositiveInteger,
    /// Bake or extract.
    BakeOrExtract,
    /// Exact source-transform path.
    SourceTransformPath,
    /// Bounded ordered text list.
    TextList,
    /// Closed semantic token.
    Token,
    /// Unreal sample-rate selection.
    SampleRate,
}

/// Unreal sample-rate selection without hidden compound values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineSampleRateV2 {
    /// `bUseDefaultSampleRate=true` and no hidden custom rate.
    Default30,
    /// `bUseDefaultSampleRate=false, CustomSampleRate=0`.
    SourceDetermined,
    /// Explicit custom rate in hertz.
    CustomHz(u32),
}

/// Closed materialized setting values.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineSettingValueV2 {
    /// Boolean value.
    Boolean(bool),
    /// Positive integer value.
    PositiveInteger(u32),
    /// Bake/extract value.
    BakeOrExtract(EngineBakeOrExtractV1),
    /// Exact case-sensitive source path.
    SourceTransformPath(String),
    /// Ordered bounded text list.
    TextList(#[serde(deserialize_with = "deserialize_collection_vec")] Vec<String>),
    /// Closed semantic token.
    Token(String),
    /// Unreal sample-rate policy.
    SampleRate(EngineSampleRateV2),
}

impl EngineSettingValueV2 {
    pub(crate) fn retained_text_bytes(&self) -> Result<usize, EngineContractError> {
        match self {
            Self::SourceTransformPath(value) | Self::Token(value) => Ok(value.len()),
            Self::TextList(values) => checked_sum(
                "V2 setting value retained text",
                values.iter().map(String::len),
            ),
            Self::Boolean(_)
            | Self::PositiveInteger(_)
            | Self::BakeOrExtract(_)
            | Self::SampleRate(_) => Ok(0),
        }
    }
}

/// Immutable V2 setting descriptor with exact source applicability and default.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineSettingDescriptorV2 {
    id: EngineSettingIdV2,
    scope: EngineSettingScopeV1,
    domain: EngineSettingDomainV2,
    #[serde(deserialize_with = "deserialize_collection_vec")]
    applicable_source_formats: Vec<SourceFormatV1>,
    default_value: Option<EngineSettingValueV2>,
}

impl EngineSettingDescriptorV2 {
    /// Construct one descriptor. An empty format set means not applicable.
    pub fn new(
        id: EngineSettingIdV2,
        scope: EngineSettingScopeV1,
        domain: EngineSettingDomainV2,
        mut applicable_source_formats: Vec<SourceFormatV1>,
        default_value: Option<EngineSettingValueV2>,
    ) -> Result<Self, EngineContractError> {
        applicable_source_formats.sort_by_key(|format| source_format_name(*format));
        applicable_source_formats.dedup();
        let descriptor = Self {
            id,
            scope,
            domain,
            applicable_source_formats,
            default_value,
        };
        descriptor.validate()?;
        Ok(descriptor)
    }

    /// Stable setting id.
    pub const fn id(&self) -> EngineSettingIdV2 {
        self.id
    }

    /// Materialization scope.
    pub const fn scope(&self) -> EngineSettingScopeV1 {
        self.scope
    }

    /// Closed value domain.
    pub const fn domain(&self) -> EngineSettingDomainV2 {
        self.domain
    }

    /// Exact source-format applicability.
    pub fn applicable_source_formats(&self) -> &[SourceFormatV1] {
        &self.applicable_source_formats
    }

    /// Verified profile default, if one exists.
    pub const fn default_value(&self) -> Option<&EngineSettingValueV2> {
        self.default_value.as_ref()
    }

    fn validate(&self) -> Result<(), EngineContractError> {
        validate_collection_len(
            "V2 descriptor.applicable_source_formats",
            self.applicable_source_formats.len(),
        )?;
        validate_unique_order(
            "V2 descriptor.applicable_source_formats",
            &self.applicable_source_formats,
            |format| source_format_name(*format),
            true,
        )?;
        if self.applicable_source_formats.is_empty() && self.default_value.is_some() {
            return Err(EngineContractError::InvalidV2DescriptorDefault { setting: self.id });
        }
        if let Some(value) = &self.default_value {
            validate_setting_value_v2(self.id, self.domain, value)?;
        }
        Ok(())
    }
}

/// One primary source for V2 facts and settings.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnginePrimarySourceV2 {
    id: String,
    target_version: String,
    url: String,
    verified_on: String,
    #[serde(deserialize_with = "deserialize_collection_vec")]
    supported_fact_ids: Vec<EngineFactIdV2>,
    #[serde(deserialize_with = "deserialize_collection_vec")]
    supported_setting_ids: Vec<EngineSettingIdV2>,
}

impl EnginePrimarySourceV2 {
    /// Construct one primary-source support row.
    pub fn new(
        id: impl Into<String>,
        target_version: impl Into<String>,
        url: impl Into<String>,
        verified_on: impl Into<String>,
        mut supported_fact_ids: Vec<EngineFactIdV2>,
        mut supported_setting_ids: Vec<EngineSettingIdV2>,
    ) -> Result<Self, EngineContractError> {
        supported_fact_ids.sort_by_key(|id| id.as_str());
        supported_fact_ids.dedup();
        supported_setting_ids.sort_by_key(|id| id.as_str());
        supported_setting_ids.dedup();
        let source = Self {
            id: id.into(),
            target_version: target_version.into(),
            url: url.into(),
            verified_on: verified_on.into(),
            supported_fact_ids,
            supported_setting_ids,
        };
        source.validate()?;
        Ok(source)
    }

    /// Stable source id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Fact ids supported by this source.
    pub fn supported_fact_ids(&self) -> &[EngineFactIdV2] {
        &self.supported_fact_ids
    }

    /// Setting ids supported by this source.
    pub fn supported_setting_ids(&self) -> &[EngineSettingIdV2] {
        &self.supported_setting_ids
    }

    fn validate(&self) -> Result<(), EngineContractError> {
        for (field, value) in [
            ("V2 primary source.id", self.id.as_str()),
            (
                "V2 primary source.target_version",
                self.target_version.as_str(),
            ),
            ("V2 primary source.url", self.url.as_str()),
            ("V2 primary source.verified_on", self.verified_on.as_str()),
        ] {
            validate_required_text(field, value)?;
        }
        validate_collection_len(
            "V2 primary source.supported_fact_ids",
            self.supported_fact_ids.len(),
        )?;
        validate_collection_len(
            "V2 primary source.supported_setting_ids",
            self.supported_setting_ids.len(),
        )?;
        validate_unique_order(
            "V2 primary source.supported_fact_ids",
            &self.supported_fact_ids,
            |id| id.as_str(),
            true,
        )?;
        validate_unique_order(
            "V2 primary source.supported_setting_ids",
            &self.supported_setting_ids,
            |id| id.as_str(),
            true,
        )
    }
}

/// Self-contained immutable V2 engine profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedEngineProfileV2 {
    schema: &'static str,
    selection: EngineProfileSelectionV1,
    fact_bundle_urn: String,
    identity: InputIdentity,
    facts: Vec<EngineProfileFactV2>,
    setting_descriptors: Vec<EngineSettingDescriptorV2>,
    primary_sources: Vec<EnginePrimarySourceV2>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolvedEngineProfileWireV2 {
    schema: String,
    selection: EngineProfileSelectionV1,
    fact_bundle_urn: String,
    identity: InputIdentity,
    #[serde(deserialize_with = "deserialize_collection_rows")]
    facts: CappedSequence<EngineProfileFactV2>,
    #[serde(deserialize_with = "deserialize_collection_rows")]
    setting_descriptors: CappedSequence<EngineSettingDescriptorV2>,
    #[serde(deserialize_with = "deserialize_collection_rows")]
    primary_sources: CappedSequence<EnginePrimarySourceV2>,
}

impl ResolvedEngineProfileV2 {
    /// Construct and canonically order one V2 profile.
    pub fn new(
        selection: EngineProfileSelectionV1,
        fact_bundle_urn: impl Into<String>,
        mut facts: Vec<EngineProfileFactV2>,
        mut setting_descriptors: Vec<EngineSettingDescriptorV2>,
        mut primary_sources: Vec<EnginePrimarySourceV2>,
    ) -> Result<Self, EngineContractError> {
        for fact in &mut facts {
            if let EngineFactStateV2::Known(EngineFactValueV2::AcceptedFormats(formats)) =
                &mut fact.state
            {
                formats.sort_by_key(|format| source_format_name(*format));
                formats.dedup();
            }
        }
        facts.sort_by_key(|fact| fact.id.as_str());
        setting_descriptors.sort_by_key(|descriptor| descriptor.id.as_str());
        primary_sources.sort_by(|left, right| left.id.cmp(&right.id));
        let mut profile = Self {
            schema: ENGINE_PROFILE_FACTS_V2_ID,
            selection,
            fact_bundle_urn: fact_bundle_urn.into(),
            identity: InputIdentity::from_bytes(&[]),
            facts,
            setting_descriptors,
            primary_sources,
        };
        profile.validate_semantics(false)?;
        profile.identity = profile.computed_identity();
        Ok(profile)
    }

    /// Immutable contract id.
    pub const fn contract_id(&self) -> &'static str {
        self.schema
    }

    /// Exact selected tuple.
    pub const fn selection(&self) -> &EngineProfileSelectionV1 {
        &self.selection
    }

    /// Fact bundle URN.
    pub fn fact_bundle_urn(&self) -> &str {
        &self.fact_bundle_urn
    }

    /// Canonical profile identity.
    pub const fn facts_identity(&self) -> &InputIdentity {
        &self.identity
    }

    /// Canonically ordered complete fact inventory.
    pub fn facts(&self) -> &[EngineProfileFactV2] {
        &self.facts
    }

    /// Canonically ordered setting descriptors.
    pub fn setting_descriptors(&self) -> &[EngineSettingDescriptorV2] {
        &self.setting_descriptors
    }

    /// Canonically ordered primary sources.
    pub fn primary_sources(&self) -> &[EnginePrimarySourceV2] {
        &self.primary_sources
    }

    /// Look up one fact.
    pub fn fact(&self, id: EngineFactIdV2) -> Option<&EngineProfileFactV2> {
        self.facts.iter().find(|fact| fact.id == id)
    }

    /// Look up one descriptor.
    pub fn setting_descriptor(&self, id: EngineSettingIdV2) -> Option<&EngineSettingDescriptorV2> {
        self.setting_descriptors.iter().find(|row| row.id == id)
    }

    /// Look up one primary source.
    pub fn source(&self, id: &str) -> Option<&EnginePrimarySourceV2> {
        self.primary_sources.iter().find(|source| source.id == id)
    }

    /// Whether the exact accepted-inputs fact admits a format.
    pub fn accepts_format(&self, format: SourceFormatV1) -> bool {
        matches!(
            self.fact(EngineFactIdV2::AcceptedInputs).map(|fact| fact.state()),
            Some(EngineFactStateV2::Known(EngineFactValueV2::AcceptedFormats(formats)))
                if formats.contains(&format)
        )
    }

    /// Revalidate canonical form and identity.
    pub fn validate(&self) -> Result<(), EngineContractError> {
        self.validate_semantics(true)
    }

    pub(crate) fn retained_text_bytes(&self) -> Result<usize, EngineContractError> {
        let fact_text = self.facts.iter().map(|fact| match fact.state() {
            EngineFactStateV2::Known(EngineFactValueV2::Token(value)) => value.len(),
            _ => 0,
        });
        let descriptor_text = self.setting_descriptors.iter().map(|descriptor| {
            descriptor
                .default_value()
                .map_or(Ok(0), EngineSettingValueV2::retained_text_bytes)
        });
        let source_text = self.primary_sources.iter().map(|source| {
            checked_sum(
                "V2 primary-source retained text",
                [
                    source.id.len(),
                    source.target_version.len(),
                    source.url.len(),
                    source.verified_on.len(),
                ],
            )
        });
        checked_sum(
            "V2 profile retained text",
            [
                self.selection.retained_text_bytes()?,
                self.fact_bundle_urn.len(),
                checked_sum("V2 fact retained text", fact_text)?,
                checked_sum(
                    "V2 descriptor retained text",
                    descriptor_text.collect::<Result<Vec<_>, _>>()?,
                )?,
                checked_sum(
                    "V2 source retained text",
                    source_text.collect::<Result<Vec<_>, _>>()?,
                )?,
            ],
        )
    }

    pub(crate) fn encode_preimage(&self, encoder: &mut CanonicalEncoder) {
        encoder.token(ENGINE_FACTS_V2_PREIMAGE_DOMAIN);
        encode_profile_key(encoder, &self.selection);
        encoder.field("fact_bundle_urn");
        encoder.token(&self.fact_bundle_urn);
        encode_json_rows(encoder, "facts", &self.facts);
        encode_json_rows(encoder, "setting_descriptors", &self.setting_descriptors);
        encode_json_rows(encoder, "primary_sources", &self.primary_sources);
    }

    fn computed_identity(&self) -> InputIdentity {
        let mut encoder = CanonicalEncoder::default();
        self.encode_preimage(&mut encoder);
        encoder.identity()
    }

    fn validate_semantics(&self, verify_identity: bool) -> Result<(), EngineContractError> {
        validate_schema("V2 profile.schema", self.schema, ENGINE_PROFILE_FACTS_V2_ID)?;
        self.selection.validate()?;
        validate_required_text("V2 profile.fact_bundle_urn", &self.fact_bundle_urn)?;
        for (field, len) in [
            ("V2 profile.facts", self.facts.len()),
            (
                "V2 profile.setting_descriptors",
                self.setting_descriptors.len(),
            ),
            ("V2 profile.primary_sources", self.primary_sources.len()),
        ] {
            validate_collection_len(field, len)?;
        }
        validate_unique_order("V2 profile.facts", &self.facts, |row| row.id.as_str(), true)?;
        if self.facts.len() != ALL_FACT_IDS_V2.len()
            || !self
                .facts
                .iter()
                .zip(ALL_FACT_IDS_V2)
                .all(|(row, expected)| row.id == expected)
        {
            return Err(EngineContractError::InvalidV2FactInventory);
        }
        for fact in &self.facts {
            validate_fact_value_v2(fact)?;
        }
        validate_unique_order(
            "V2 profile.setting_descriptors",
            &self.setting_descriptors,
            |row| row.id.as_str(),
            true,
        )?;
        for descriptor in &self.setting_descriptors {
            descriptor.validate()?;
        }
        validate_unique_order(
            "V2 profile.primary_sources",
            &self.primary_sources,
            |row| row.id.as_str(),
            true,
        )?;
        for source in &self.primary_sources {
            source.validate()?;
            for id in source.supported_fact_ids() {
                if !matches!(
                    self.fact(*id).map(|fact| fact.state()),
                    Some(EngineFactStateV2::Known(_))
                ) {
                    return Err(EngineContractError::InvalidV2SourceFact {
                        source_id: source.id.clone(),
                        fact: *id,
                    });
                }
            }
            for id in source.supported_setting_ids() {
                if self.setting_descriptor(*id).is_none() {
                    return Err(EngineContractError::InvalidV2SourceSetting {
                        source_id: source.id.clone(),
                        setting: *id,
                    });
                }
            }
        }
        for fact in &self.facts {
            if matches!(fact.state(), EngineFactStateV2::Known(_))
                && !self
                    .primary_sources
                    .iter()
                    .any(|source| source.supported_fact_ids.contains(&fact.id))
            {
                return Err(EngineContractError::UnreferencedV2Fact { fact: fact.id });
            }
        }
        for descriptor in &self.setting_descriptors {
            if !self
                .primary_sources
                .iter()
                .any(|source| source.supported_setting_ids.contains(&descriptor.id))
            {
                return Err(EngineContractError::UnreferencedV2Setting {
                    setting: descriptor.id,
                });
            }
        }
        if !matches!(
            self.fact(EngineFactIdV2::AcceptedInputs).map(|fact| fact.state()),
            Some(EngineFactStateV2::Known(EngineFactValueV2::AcceptedFormats(formats))) if !formats.is_empty()
        ) {
            return Err(EngineContractError::InvalidAcceptedInputs);
        }
        let rows = self
            .facts
            .len()
            .checked_add(self.setting_descriptors.len())
            .and_then(|rows| rows.checked_add(self.primary_sources.len()))
            .ok_or(EngineContractError::ArithmeticOverflow {
                field: "V2 profile rows",
            })?;
        if rows > ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS {
            return Err(EngineContractError::TooManyAggregateRows {
                found: rows,
                max: ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS,
            });
        }
        let json_bytes = serde_json::to_vec(&(
            &self.selection,
            &self.fact_bundle_urn,
            &self.facts,
            &self.setting_descriptors,
            &self.primary_sources,
        ))
        .expect("V2 profile has infallible JSON serialization")
        .len();
        if json_bytes > ENGINE_CONTRACT_V1_MAX_TOTAL_TEXT_BYTES {
            return Err(EngineContractError::TooMuchAggregateText {
                found: json_bytes,
                max: ENGINE_CONTRACT_V1_MAX_TOTAL_TEXT_BYTES,
            });
        }
        if verify_identity && self.identity != self.computed_identity() {
            return Err(EngineContractError::IdentityMismatch {
                contract: ENGINE_PROFILE_FACTS_V2_ID,
            });
        }
        Ok(())
    }
}

impl TryFrom<ResolvedEngineProfileWireV2> for ResolvedEngineProfileV2 {
    type Error = EngineContractError;

    fn try_from(wire: ResolvedEngineProfileWireV2) -> Result<Self, Self::Error> {
        if wire.facts.overflowed
            || wire.setting_descriptors.overflowed
            || wire.primary_sources.overflowed
        {
            return Err(EngineContractError::TooManyRows {
                field: "V2 profile collection",
                found: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
            });
        }
        let profile = Self {
            schema: if wire.schema == ENGINE_PROFILE_FACTS_V2_ID {
                ENGINE_PROFILE_FACTS_V2_ID
            } else {
                return Err(EngineContractError::InvalidSchema {
                    field: "V2 profile.schema",
                    expected: ENGINE_PROFILE_FACTS_V2_ID,
                    found: wire.schema,
                });
            },
            selection: wire.selection,
            fact_bundle_urn: wire.fact_bundle_urn,
            identity: wire.identity,
            facts: wire.facts.values,
            setting_descriptors: wire.setting_descriptors.values,
            primary_sources: wire.primary_sources.values,
        };
        profile.validate()?;
        Ok(profile)
    }
}

impl<'de> Deserialize<'de> for ResolvedEngineProfileV2 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        ResolvedEngineProfileWireV2::deserialize(deserializer)?
            .try_into()
            .map_err(D::Error::custom)
    }
}

/// Authority that supplied one resolved value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineSettingValueOriginV3 {
    /// Caller configuration declared the value.
    ExplicitConfig,
    /// The immutable profile supplied its verified default.
    ProfileDefault,
}

/// One materialized V3 setting row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineSettingRowV3 {
    id: EngineSettingIdV2,
    value: EngineSettingValueV2,
    value_origin: EngineSettingValueOriginV3,
}

impl EngineSettingRowV3 {
    /// Construct one origin-bearing row.
    pub const fn new(
        id: EngineSettingIdV2,
        value: EngineSettingValueV2,
        value_origin: EngineSettingValueOriginV3,
    ) -> Self {
        Self {
            id,
            value,
            value_origin,
        }
    }

    /// Stable setting id.
    pub const fn id(&self) -> EngineSettingIdV2 {
        self.id
    }
    /// Materialized value.
    pub const fn value(&self) -> &EngineSettingValueV2 {
        &self.value
    }
    /// Exact value authority.
    pub const fn value_origin(&self) -> EngineSettingValueOriginV3 {
        self.value_origin
    }
}

/// Origin-bearing settings for one exact source clip ordinal/name pair.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineClipSettingsV3 {
    clip_ordinal: u64,
    clip_name: String,
    #[serde(deserialize_with = "deserialize_collection_vec")]
    settings: Vec<EngineSettingRowV3>,
}

impl EngineClipSettingsV3 {
    /// Construct one clip row and canonicalize its settings.
    pub fn new(
        clip_ordinal: u64,
        clip_name: impl Into<String>,
        mut settings: Vec<EngineSettingRowV3>,
    ) -> Result<Self, EngineContractError> {
        settings.sort_by_key(|row| row.id.as_str());
        let row = Self {
            clip_ordinal,
            clip_name: clip_name.into(),
            settings,
        };
        validate_required_text("V3 settings.clip_name", &row.clip_name)?;
        validate_collection_len("V3 settings.clip.settings", row.settings.len())?;
        validate_unique_order(
            "V3 settings.clip.settings",
            &row.settings,
            |setting| setting.id.as_str(),
            true,
        )?;
        Ok(row)
    }

    /// Original zero-based source clip ordinal.
    pub const fn clip_ordinal(&self) -> u64 {
        self.clip_ordinal
    }
    /// Exact source clip name.
    pub fn clip_name(&self) -> &str {
        &self.clip_name
    }
    /// Canonically ordered settings.
    pub fn settings(&self) -> &[EngineSettingRowV3] {
        &self.settings
    }
    /// Look up one setting.
    pub fn setting(&self, id: EngineSettingIdV2) -> Option<&EngineSettingRowV3> {
        self.settings.iter().find(|row| row.id == id)
    }
}

/// Bounded, origin-bearing resolved settings used by profile facts V2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedEngineSettingsV3 {
    schema: &'static str,
    identity: InputIdentity,
    source_format: SourceFormatV1,
    document_settings: Vec<EngineSettingRowV3>,
    clips: Vec<EngineClipSettingsV3>,
    clip_coverage: ResolvedEngineSettingsCoverageV2,
    work: ResolvedEngineSettingsWorkV2,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolvedEngineSettingsWireV3 {
    schema: String,
    identity: InputIdentity,
    source_format: SourceFormatV1,
    #[serde(deserialize_with = "deserialize_collection_rows")]
    document_settings: CappedSequence<EngineSettingRowV3>,
    #[serde(deserialize_with = "deserialize_collection_rows")]
    clips: CappedSequence<EngineClipSettingsV3>,
    clip_coverage: ResolvedEngineSettingsCoverageV2,
    work: ResolvedEngineSettingsWorkV2,
}

impl ResolvedEngineSettingsV3 {
    /// Construct complete or explicit-partial settings and bind them to a profile.
    pub fn new(
        profile: &ResolvedEngineProfileV2,
        source_format: SourceFormatV1,
        mut document_settings: Vec<EngineSettingRowV3>,
        mut clips: Vec<EngineClipSettingsV3>,
        clip_coverage: ResolvedEngineSettingsCoverageV2,
        work: ResolvedEngineSettingsWorkV2,
    ) -> Result<Self, EngineContractError> {
        document_settings.sort_by_key(|row| row.id.as_str());
        clips.sort_by(|left, right| {
            (left.clip_ordinal, left.clip_name.as_str())
                .cmp(&(right.clip_ordinal, right.clip_name.as_str()))
        });
        let mut settings = Self {
            schema: RESOLVED_ENGINE_SETTINGS_V3_ID,
            identity: InputIdentity::from_bytes(&[]),
            source_format,
            document_settings,
            clips,
            clip_coverage,
            work,
        };
        settings.validate_semantics(profile, false)?;
        settings.identity = settings.computed_identity(profile);
        Ok(settings)
    }

    /// Immutable contract id.
    pub const fn contract_id(&self) -> &'static str {
        self.schema
    }
    /// Canonical settings identity.
    pub const fn settings_identity(&self) -> &InputIdentity {
        &self.identity
    }
    /// Exact source format whose applicable descriptors were materialized.
    pub const fn source_format(&self) -> SourceFormatV1 {
        self.source_format
    }
    /// Document-scoped rows.
    pub fn document_settings(&self) -> &[EngineSettingRowV3] {
        &self.document_settings
    }
    /// Clip-scoped rows in source-ordinal order.
    pub fn clips(&self) -> &[EngineClipSettingsV3] {
        &self.clips
    }
    /// Exact/partial clip coverage.
    pub const fn clip_coverage(&self) -> &ResolvedEngineSettingsCoverageV2 {
        &self.clip_coverage
    }
    /// Bounded work counters.
    pub const fn work(&self) -> ResolvedEngineSettingsWorkV2 {
        self.work
    }
    /// Look up one document setting.
    pub fn document_setting(&self, id: EngineSettingIdV2) -> Option<&EngineSettingRowV3> {
        self.document_settings.iter().find(|row| row.id == id)
    }
    /// Look up one exact clip row.
    pub fn clip_row(&self, ordinal: u64, name: &str) -> Option<&EngineClipSettingsV3> {
        self.clips
            .iter()
            .find(|row| row.clip_ordinal == ordinal && row.clip_name == name)
    }
    /// Validate against the exact profile used by the identity.
    pub fn validate_against(
        &self,
        profile: &ResolvedEngineProfileV2,
    ) -> Result<(), EngineContractError> {
        self.validate_semantics(profile, true)
    }

    pub(crate) fn retained_text_bytes(&self) -> Result<usize, EngineContractError> {
        let document_text = self
            .document_settings
            .iter()
            .map(|row| row.value.retained_text_bytes())
            .collect::<Result<Vec<_>, _>>()?;
        let clip_text = self
            .clips
            .iter()
            .map(|clip| {
                checked_sum(
                    "V3 clip retained text",
                    std::iter::once(clip.clip_name.len()).chain(
                        clip.settings
                            .iter()
                            .map(|row| row.value.retained_text_bytes())
                            .collect::<Result<Vec<_>, _>>()?,
                    ),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        checked_sum(
            "V3 settings retained text",
            document_text.into_iter().chain(clip_text),
        )
    }

    pub(crate) fn encode_preimage(
        &self,
        profile: &ResolvedEngineProfileV2,
        encoder: &mut CanonicalEncoder,
    ) {
        encoder.token(ENGINE_SETTINGS_V3_PREIMAGE_DOMAIN);
        encoder.field("profile_identity");
        encode_input_identity(encoder, profile.facts_identity());
        encoder.field("source_format");
        encoder.token(source_format_name(self.source_format));
        encode_json_rows(encoder, "document_settings", &self.document_settings);
        encode_json_rows(encoder, "clips", &self.clips);
        encoder.field("clip_coverage");
        encoder.token(serde_json::to_string(&self.clip_coverage).expect("coverage serializes"));
        encoder.field("work");
        encoder.token(serde_json::to_string(&self.work).expect("work serializes"));
    }

    fn computed_identity(&self, profile: &ResolvedEngineProfileV2) -> InputIdentity {
        let mut encoder = CanonicalEncoder::default();
        self.encode_preimage(profile, &mut encoder);
        encoder.identity()
    }

    fn validate_semantics(
        &self,
        profile: &ResolvedEngineProfileV2,
        verify_identity: bool,
    ) -> Result<(), EngineContractError> {
        profile.validate()?;
        if !profile.accepts_format(self.source_format) {
            return Err(EngineContractError::InvalidV3SettingsSourceFormat {
                format: self.source_format,
            });
        }
        validate_schema(
            "V3 settings.schema",
            self.schema,
            RESOLVED_ENGINE_SETTINGS_V3_ID,
        )?;
        for (field, len) in [
            (
                "V3 settings.document_settings",
                self.document_settings.len(),
            ),
            ("V3 settings.clips", self.clips.len()),
        ] {
            validate_collection_len(field, len)?;
        }
        validate_unique_order(
            "V3 settings.document_settings",
            &self.document_settings,
            |row| row.id.as_str(),
            true,
        )?;
        if self.clips.windows(2).any(|pair| {
            (pair[0].clip_ordinal, pair[0].clip_name.as_str())
                >= (pair[1].clip_ordinal, pair[1].clip_name.as_str())
        }) {
            return Err(EngineContractError::NonCanonicalOrder {
                field: "V3 settings.clips",
            });
        }
        for (expected_ordinal, clip) in self.clips.iter().enumerate() {
            if clip.clip_ordinal != expected_ordinal as u64 {
                return Err(EngineContractError::NonCanonicalOrder {
                    field: "V3 settings.clips source ordinals",
                });
            }
            validate_required_text("V3 settings.clip_name", &clip.clip_name)?;
            validate_collection_len("V3 settings.clip.settings", clip.settings.len())?;
            validate_unique_order(
                "V3 settings.clip.settings",
                &clip.settings,
                |row| row.id.as_str(),
                true,
            )?;
        }
        for row in self
            .document_settings
            .iter()
            .chain(self.clips.iter().flat_map(|clip| clip.settings.iter()))
        {
            let descriptor = profile
                .setting_descriptor(row.id)
                .ok_or(EngineContractError::UnknownV2MaterializedSetting { setting: row.id })?;
            let expected_scope = if self
                .document_settings
                .iter()
                .any(|candidate| std::ptr::eq(candidate, row))
            {
                EngineSettingScopeV1::Document
            } else {
                EngineSettingScopeV1::Clip
            };
            if descriptor.scope != expected_scope {
                return Err(EngineContractError::WrongV2SettingScope { setting: row.id });
            }
            if !descriptor
                .applicable_source_formats
                .contains(&self.source_format)
            {
                return Err(EngineContractError::InapplicableV2MaterializedSetting {
                    setting: row.id,
                    format: self.source_format,
                });
            }
            validate_setting_value_v2(row.id, descriptor.domain, &row.value)?;
            if row.value_origin == EngineSettingValueOriginV3::ProfileDefault
                && descriptor.default_value.as_ref() != Some(&row.value)
            {
                return Err(EngineContractError::InvalidProfileDefaultOrigin { setting: row.id });
            }
        }
        for descriptor in profile.setting_descriptors().iter().filter(|descriptor| {
            descriptor
                .applicable_source_formats
                .contains(&self.source_format)
        }) {
            match descriptor.scope {
                EngineSettingScopeV1::Document => {
                    if self.document_setting(descriptor.id).is_none() {
                        return Err(EngineContractError::MissingApplicableV2Setting {
                            setting: descriptor.id,
                            format: self.source_format,
                        });
                    }
                }
                EngineSettingScopeV1::Clip => {
                    if self
                        .clips
                        .iter()
                        .any(|clip| clip.setting(descriptor.id).is_none())
                    {
                        return Err(EngineContractError::MissingApplicableV2Setting {
                            setting: descriptor.id,
                            format: self.source_format,
                        });
                    }
                }
            }
        }
        let retained_clip_rows = self.clips.len();
        match self.clip_coverage.state() {
            ResolvedEngineSettingsCoverageStateV2::Complete
                if self.work.actual_clip_rows_inspected() != retained_clip_rows
                    || self.work.materialized_clip_rows() != retained_clip_rows
                    || self.work.retained_clip_rows() != retained_clip_rows =>
            {
                return Err(EngineContractError::InvalidV2CoverageWork);
            }
            ResolvedEngineSettingsCoverageStateV2::Partial
                if self.work.actual_clip_rows_inspected()
                    <= ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS
                    || self.work.materialized_clip_rows() != retained_clip_rows
                    || self.work.retained_clip_rows() != retained_clip_rows =>
            {
                return Err(EngineContractError::InvalidV2CoverageWork);
            }
            _ => {}
        }
        let rows = self
            .document_settings
            .len()
            .checked_add(self.clips.len())
            .and_then(|rows| {
                rows.checked_add(self.clips.iter().map(|clip| clip.settings.len()).sum())
            })
            .ok_or(EngineContractError::ArithmeticOverflow {
                field: "V3 settings rows",
            })?;
        if rows > ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS {
            return Err(EngineContractError::TooManyAggregateRows {
                found: rows,
                max: ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS,
            });
        }
        if verify_identity && self.identity != self.computed_identity(profile) {
            return Err(EngineContractError::IdentityMismatch {
                contract: RESOLVED_ENGINE_SETTINGS_V3_ID,
            });
        }
        Ok(())
    }
}

impl TryFrom<ResolvedEngineSettingsWireV3> for ResolvedEngineSettingsV3 {
    type Error = EngineContractError;

    fn try_from(wire: ResolvedEngineSettingsWireV3) -> Result<Self, Self::Error> {
        if wire.schema != RESOLVED_ENGINE_SETTINGS_V3_ID {
            return Err(EngineContractError::InvalidSchema {
                field: "V3 settings.schema",
                expected: RESOLVED_ENGINE_SETTINGS_V3_ID,
                found: wire.schema,
            });
        }
        if wire.document_settings.overflowed || wire.clips.overflowed {
            return Err(EngineContractError::TooManyRows {
                field: "V3 settings collection",
                found: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
            });
        }
        let settings = Self {
            schema: RESOLVED_ENGINE_SETTINGS_V3_ID,
            identity: wire.identity,
            source_format: wire.source_format,
            document_settings: wire.document_settings.values,
            clips: wire.clips.values,
            clip_coverage: wire.clip_coverage,
            work: wire.work,
        };
        // Profile-dependent identity and descriptor checks are deliberately
        // deferred to `validate_against`, as in historical settings readers.
        for (field, len) in [
            (
                "V3 settings.document_settings",
                settings.document_settings.len(),
            ),
            ("V3 settings.clips", settings.clips.len()),
        ] {
            validate_collection_len(field, len)?;
        }
        validate_unique_order(
            "V3 settings.document_settings",
            &settings.document_settings,
            |row| row.id.as_str(),
            true,
        )?;
        Ok(settings)
    }
}

impl<'de> Deserialize<'de> for ResolvedEngineSettingsV3 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        ResolvedEngineSettingsWireV3::deserialize(deserializer)?
            .try_into()
            .map_err(D::Error::custom)
    }
}

fn encode_json_rows<T: Serialize>(encoder: &mut CanonicalEncoder, field: &'static str, rows: &[T]) {
    encoder.field(field);
    encoder.count(rows.len());
    for row in rows {
        encoder.token(
            serde_json::to_string(row).expect("contract row has infallible JSON serialization"),
        );
    }
}

fn validate_fact_value_v2(fact: &EngineProfileFactV2) -> Result<(), EngineContractError> {
    let valid = match (&fact.id, &fact.state) {
        (_, EngineFactStateV2::Unknown | EngineFactStateV2::NotApplicable) => true,
        (
            EngineFactIdV2::AcceptedInputs,
            EngineFactStateV2::Known(EngineFactValueV2::AcceptedFormats(formats)),
        ) => {
            !formats.is_empty()
                && formats.len() <= 3
                && formats
                    .windows(2)
                    .all(|pair| source_format_name(pair[0]) < source_format_name(pair[1]))
        }
        (
            EngineFactIdV2::TargetLinearUnit,
            EngineFactStateV2::Known(EngineFactValueV2::LinearUnit(_)),
        )
        | (
            EngineFactIdV2::SourceToTargetUnitMapping,
            EngineFactStateV2::Known(EngineFactValueV2::UnitRatio(_)),
        )
        | (
            EngineFactIdV2::PhysicalDimensionsPreserved
            | EngineFactIdV2::ApplicationWorldUnitPolicy,
            EngineFactStateV2::Known(EngineFactValueV2::Boolean(_)),
        )
        | (
            EngineFactIdV2::ImporterScaleConversion
            | EngineFactIdV2::ResultingTransformScale
            | EngineFactIdV2::SourceImportDisposition
            | EngineFactIdV2::ImportSettingProjection,
            EngineFactStateV2::Known(EngineFactValueV2::Token(_)),
        )
        | (
            EngineFactIdV2::RootMotionAddressability,
            EngineFactStateV2::Known(EngineFactValueV2::RootMotionAddressability(_)),
        ) => true,
        _ => false,
    };
    if !valid {
        return Err(EngineContractError::InvalidV2FactValue { fact: fact.id });
    }
    if let EngineFactStateV2::Known(EngineFactValueV2::Token(value)) = &fact.state {
        validate_required_text("V2 fact token", value)?;
        let allowed = match fact.id {
            EngineFactIdV2::ImporterScaleConversion => value == "none",
            EngineFactIdV2::ResultingTransformScale => {
                value
                    == "loader_entities_unit_orthonormal_trs_nodes_passthrough_matrix_nodes_decomposed"
            }
            EngineFactIdV2::SourceImportDisposition => value == "materialized_import_gates",
            EngineFactIdV2::ImportSettingProjection => {
                matches!(value.as_str(), "godot_params" | "unreal_fbx_import_data")
            }
            _ => false,
        };
        if !allowed {
            return Err(EngineContractError::InvalidV2FactValue { fact: fact.id });
        }
    }
    if let EngineFactStateV2::Known(EngineFactValueV2::UnitRatio(value)) = &fact.state {
        value.validate()?;
    }
    Ok(())
}

fn validate_setting_value_v2(
    id: EngineSettingIdV2,
    domain: EngineSettingDomainV2,
    value: &EngineSettingValueV2,
) -> Result<(), EngineContractError> {
    let valid = match (domain, value) {
        (EngineSettingDomainV2::Boolean, EngineSettingValueV2::Boolean(_))
        | (EngineSettingDomainV2::BakeOrExtract, EngineSettingValueV2::BakeOrExtract(_))
        | (
            EngineSettingDomainV2::SampleRate,
            EngineSettingValueV2::SampleRate(
                EngineSampleRateV2::Default30 | EngineSampleRateV2::SourceDetermined,
            ),
        ) => true,
        (EngineSettingDomainV2::PositiveInteger, EngineSettingValueV2::PositiveInteger(value)) => {
            *value > 0
        }
        (
            EngineSettingDomainV2::SourceTransformPath,
            EngineSettingValueV2::SourceTransformPath(value),
        ) => validate_required_text("V2 setting value", value).is_ok(),
        (EngineSettingDomainV2::Token, EngineSettingValueV2::Token(value)) => {
            validate_required_text("V2 setting value", value).is_ok()
                && match id {
                    EngineSettingIdV2::LoadMeshes => {
                        matches!(value.as_str(), "empty" | "nonempty")
                    }
                    EngineSettingIdV2::ExtensionHandlerEnvironment => {
                        matches!(value.as_str(), "bare_empty" | "bevy_pbr_stock_0_19")
                    }
                    EngineSettingIdV2::AnimationType => {
                        matches!(value.as_str(), "generic" | "humanoid" | "legacy")
                    }
                    EngineSettingIdV2::AvatarSetup => matches!(
                        value.as_str(),
                        "create_from_this_model" | "copy_from_other_avatar"
                    ),
                    _ => false,
                }
        }
        (EngineSettingDomainV2::TextList, EngineSettingValueV2::TextList(values)) => {
            values.len() <= ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS
                && values
                    .iter()
                    .all(|value| validate_required_text("V2 setting list value", value).is_ok())
        }
        (
            EngineSettingDomainV2::SampleRate,
            EngineSettingValueV2::SampleRate(EngineSampleRateV2::CustomHz(value)),
        ) => (1..=48_000).contains(value),
        _ => false,
    };
    if !valid {
        return Err(EngineContractError::WrongV2SettingDomain { setting: id });
    }
    if id == EngineSettingIdV2::AnimationFps
        && !matches!(value, EngineSettingValueV2::PositiveInteger(1..=120))
    {
        return Err(EngineContractError::V2SettingValueOutOfRange { setting: id });
    }
    Ok(())
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
    /// V2 clip coverage and bounded work counters disagreed with retained rows.
    #[error("resolved-engine-settings V2 coverage and work counters are inconsistent")]
    InvalidV2CoverageWork,
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
    /// A rational was zero, had a zero denominator, or was not reduced.
    #[error("invalid reduced ratio {numerator}/{denominator}")]
    InvalidReducedRatio {
        /// Supplied numerator.
        numerator: u64,
        /// Supplied denominator.
        denominator: u64,
    },
    /// The complete closed V2 fact inventory was malformed.
    #[error("profile facts must contain every V2 fact id exactly once")]
    InvalidV2FactInventory,
    /// A V2 fact carried a value from another domain.
    #[error("V2 profile fact {fact:?} carries an invalid known-value variant")]
    InvalidV2FactValue {
        /// Invalid fact id.
        fact: EngineFactIdV2,
    },
    /// A V2 descriptor carried a default while not applicable.
    #[error("V2 setting descriptor {setting:?} has an invalid default")]
    InvalidV2DescriptorDefault {
        /// Invalid descriptor id.
        setting: EngineSettingIdV2,
    },
    /// A V2 primary source referenced a missing or non-known fact.
    #[error("V2 primary source {source_id:?} references unsupported fact {fact:?}")]
    InvalidV2SourceFact {
        /// Source id.
        source_id: String,
        /// Invalid fact id.
        fact: EngineFactIdV2,
    },
    /// A V2 primary source referenced an absent descriptor.
    #[error("V2 primary source {source_id:?} references absent setting {setting:?}")]
    InvalidV2SourceSetting {
        /// Source id.
        source_id: String,
        /// Missing setting id.
        setting: EngineSettingIdV2,
    },
    /// A known V2 fact had no supporting source.
    #[error("known V2 profile fact {fact:?} has no primary-source reference")]
    UnreferencedV2Fact {
        /// Unsupported fact.
        fact: EngineFactIdV2,
    },
    /// A V2 descriptor had no supporting source.
    #[error("V2 setting descriptor {setting:?} has no primary-source reference")]
    UnreferencedV2Setting {
        /// Unsupported setting.
        setting: EngineSettingIdV2,
    },
    /// A V3 materialized row named no profile descriptor.
    #[error("V3 settings contain unknown setting {setting:?}")]
    UnknownV2MaterializedSetting {
        /// Unknown setting id.
        setting: EngineSettingIdV2,
    },
    /// A V3 materialized row appeared at the wrong scope.
    #[error("V3 settings contain {setting:?} at the wrong scope")]
    WrongV2SettingScope {
        /// Wrong-scope setting id.
        setting: EngineSettingIdV2,
    },
    /// A V3 value did not match its descriptor domain.
    #[error("V3 setting {setting:?} has a value outside its domain")]
    WrongV2SettingDomain {
        /// Invalid setting id.
        setting: EngineSettingIdV2,
    },
    /// A bounded setting value was outside its setting-specific range.
    #[error("V3 setting {setting:?} is outside its allowed range")]
    V2SettingValueOutOfRange {
        /// Out-of-range setting id.
        setting: EngineSettingIdV2,
    },
    /// A row claimed a profile default that disagreed with its descriptor.
    #[error("V3 setting {setting:?} claims a mismatched profile default")]
    InvalidProfileDefaultOrigin {
        /// Mismatched setting id.
        setting: EngineSettingIdV2,
    },
    /// V3 settings were bound to a format not accepted by their profile.
    #[error("V3 settings source format {format:?} is not accepted by the profile")]
    InvalidV3SettingsSourceFormat {
        /// Rejected source format.
        format: SourceFormatV1,
    },
    /// A V3 row materialized a descriptor outside its source applicability.
    #[error("V3 setting {setting:?} is not applicable to source format {format:?}")]
    InapplicableV2MaterializedSetting {
        /// Inapplicable setting.
        setting: EngineSettingIdV2,
        /// Exact source format.
        format: SourceFormatV1,
    },
    /// An applicable V2 descriptor was omitted from V3 materialization.
    #[error("V3 settings omit applicable setting {setting:?} for source format {format:?}")]
    MissingApplicableV2Setting {
        /// Missing setting.
        setting: EngineSettingIdV2,
        /// Exact source format.
        format: SourceFormatV1,
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
    fn v2_settings_identity_commits_to_complete_or_n_plus_one_coverage_and_work() {
        let profile = settings_profile("v2-settings");
        let clips: Vec<_> = (0..ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS)
            .map(|_| EngineClipSettingsV1::new("same", Vec::new()).unwrap())
            .collect();
        let complete = ResolvedEngineSettingsV2::new(
            &profile,
            document_settings(),
            clips.clone(),
            ResolvedEngineSettingsCoverageV2::complete(),
            ResolvedEngineSettingsWorkV2::new(
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
            ),
        )
        .unwrap();
        let partial = ResolvedEngineSettingsV2::new(
            &profile,
            document_settings(),
            clips,
            ResolvedEngineSettingsCoverageV2::actual_clip_rows_exceeded(),
            ResolvedEngineSettingsWorkV2::new(
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
            ),
        )
        .unwrap();

        assert_ne!(complete.settings_identity(), partial.settings_identity());
        for changed_work in [
            ResolvedEngineSettingsWorkV2::new(
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS - 1,
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
            ),
            ResolvedEngineSettingsWorkV2::new(
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS - 1,
            ),
        ] {
            let mut changed = complete.clone();
            changed.work = changed_work;
            assert_ne!(
                complete.settings_identity(),
                &changed.computed_identity(&profile),
                "every bounded work counter must change the V2 identity preimage"
            );
        }
        complete.validate_against(&profile).unwrap();
        partial.validate_against(&profile).unwrap();

        let mut forged = serde_json::to_value(&complete).unwrap();
        forged["clip_coverage"] = serde_json::json!({
            "state": "partial",
            "reason": "actual_clip_rows_exceeded"
        });
        assert!(serde_json::from_value::<ResolvedEngineSettingsV2>(forged).is_err());
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

    fn assert_profile_limit(value: &serde_json::Value, expected: EngineContractError) {
        match decode_resolved_engine_profile_v1(&serde_json::to_string(value).unwrap()) {
            Err(EngineContractDecodeError::Semantic(error)) => assert_eq!(error, expected),
            other => panic!("expected typed profile limit, got {other:?}"),
        }
    }

    fn assert_settings_limit(value: &serde_json::Value, expected: EngineContractError) {
        match decode_resolved_engine_settings_v1_with_provenance_limit(
            &serde_json::to_string(value).unwrap(),
            ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS,
        ) {
            Err(EngineSettingsLimitedDecodeError::Contract(
                EngineContractDecodeError::Semantic(error),
            )) => assert_eq!(error, expected),
            _ => panic!("expected typed settings limit"),
        }
    }

    #[test]
    fn profile_sequences_reject_n_plus_one_before_decoding_null_sentinels() {
        let profile = settings_profile("stream-profile");
        let base = serde_json::to_value(&profile).unwrap();
        for (field, element) in [
            ("facts", base["facts"][0].clone()),
            (
                "setting_descriptors",
                base["setting_descriptors"][0].clone(),
            ),
            ("primary_sources", base["primary_sources"][0].clone()),
        ] {
            let mut rows = vec![element; ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS];
            rows.push(serde_json::Value::Null);
            let mut over = base.clone();
            over[field] = rows.into();
            assert_profile_limit(
                &over,
                EngineContractError::TooManyRows {
                    field: match field {
                        "facts" => "profile.facts",
                        "setting_descriptors" => "profile.setting_descriptors",
                        "primary_sources" => "profile.primary_sources",
                        _ => unreachable!(),
                    },
                    found: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                    max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                },
            );
        }

        let mut accepted = vec![serde_json::json!("glb"); ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS];
        accepted.push(serde_json::Value::Null);
        let mut over = base.clone();
        over["facts"][0]["state"]["known"]["accepted_formats"] = accepted.into();
        assert_profile_limit(&over, EngineContractError::InvalidAcceptedInputs);

        for (field, value) in [
            ("supported_fact_ids", serde_json::json!("accepted_inputs")),
            ("supported_setting_ids", serde_json::json!("convert_units")),
        ] {
            let mut rows = vec![value; ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS];
            rows.push(serde_json::Value::Null);
            let mut over = base.clone();
            over["primary_sources"][0][field] = rows.into();
            assert_profile_limit(
                &over,
                EngineContractError::TooManyRows {
                    field: if field == "supported_fact_ids" {
                        "primary_sources.supported_fact_ids"
                    } else {
                        "primary_sources.supported_setting_ids"
                    },
                    found: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                    max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                },
            );
        }
    }

    #[test]
    fn settings_sequences_reject_n_plus_one_before_decoding_null_sentinels() {
        let profile = godot_profile();
        let settings = ResolvedEngineSettingsV1::new(&profile, vec![], vec![]).unwrap();
        let base = serde_json::to_value(settings).unwrap();
        let row = serde_json::json!({"id": "convert_units", "value": {"boolean": true}});
        let clip = serde_json::json!({"clip_name": "clip", "settings": []});

        for (field, element, error_field) in [
            (
                "document_settings",
                row.clone(),
                "settings.document_settings",
            ),
            ("clips", clip.clone(), "settings.clips"),
        ] {
            let mut rows = vec![element; ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS];
            rows.push(serde_json::Value::Null);
            let mut over = base.clone();
            over[field] = rows.into();
            assert_settings_limit(
                &over,
                EngineContractError::TooManyRows {
                    field: error_field,
                    found: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                    max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
                },
            );
        }

        let mut rows = vec![row; ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS];
        rows.push(serde_json::Value::Null);
        let mut over = base;
        over["clips"] = serde_json::json!([{"clip_name": "clip", "settings": rows}]);
        assert_settings_limit(
            &over,
            EngineContractError::TooManyRows {
                field: "settings.clips.settings",
                found: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
            },
        );
    }

    #[test]
    fn nested_profile_and_settings_aggregate_budgets_stop_at_global_n_plus_one() {
        let profile = settings_profile("aggregate-profile");
        let mut profile_wire = serde_json::to_value(profile).unwrap();
        profile_wire["facts"] = serde_json::json!([]);
        profile_wire["setting_descriptors"] = serde_json::json!([]);
        let source_template = serde_json::json!({
            "id": "source",
            "target_version": "1",
            "url": "https://example.invalid",
            "verified_on": "2026-08-20",
            "supported_fact_ids": vec![
                "accepted_inputs";
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS
            ],
            "supported_setting_ids": vec![
                "convert_units";
                ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS
            ]
        });
        let mut sources = vec![source_template.clone(); 7];
        let mut last = source_template;
        last["supported_setting_ids"] = serde_json::json!(vec!["convert_units"; 4_088]);
        last["supported_setting_ids"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::Value::Null);
        sources.push(last);
        profile_wire["primary_sources"] = sources.into();
        assert_profile_limit(
            &profile_wire,
            EngineContractError::TooManyAggregateRows {
                found: ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS + 1,
                max: ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS,
            },
        );
        let mut locally_oversized =
            vec![serde_json::json!("convert_units"); ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS];
        locally_oversized.push(serde_json::Value::Null);
        profile_wire["primary_sources"][7]["supported_setting_ids"] = locally_oversized.into();
        assert_profile_limit(
            &profile_wire,
            EngineContractError::TooManyRows {
                field: "primary_sources.supported_setting_ids",
                found: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
            },
        );

        let profile = godot_profile();
        let settings = ResolvedEngineSettingsV1::new(&profile, vec![], vec![]).unwrap();
        let mut settings_wire = serde_json::to_value(&settings).unwrap();
        let setting = serde_json::json!({"id": "convert_units", "value": {"boolean": true}});
        let full_clip = serde_json::json!({
            "clip_name": "clip",
            "settings": vec![setting.clone(); 4_095]
        });
        let mut clips = vec![full_clip; 15];
        let mut last = serde_json::json!({
            "clip_name": "clip",
            "settings": vec![setting; 4_095]
        });
        last["settings"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::Value::Null);
        clips.push(last);
        settings_wire["clips"] = clips.into();
        assert_settings_limit(
            &settings_wire,
            EngineContractError::TooManyAggregateRows {
                found: ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS + 1,
                max: ENGINE_CONTRACT_V1_MAX_AGGREGATE_ROWS,
            },
        );
        let mut locally_oversized = vec![
            serde_json::json!({"id": "convert_units", "value": {"boolean": true}});
            ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS
        ];
        locally_oversized.push(serde_json::Value::Null);
        settings_wire["clips"][15]["settings"] = locally_oversized.into();
        assert_settings_limit(
            &settings_wire,
            EngineContractError::TooManyRows {
                field: "settings.clips.settings",
                found: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS + 1,
                max: ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS,
            },
        );
    }

    fn profile_v2_for_origin_tests() -> ResolvedEngineProfileV2 {
        let mut facts = ALL_FACT_IDS_V2
            .into_iter()
            .map(|id| EngineProfileFactV2::new(id, EngineFactStateV2::Unknown))
            .collect::<Vec<_>>();
        facts
            .iter_mut()
            .find(|fact| fact.id() == EngineFactIdV2::AcceptedInputs)
            .unwrap()
            .state = EngineFactStateV2::Known(EngineFactValueV2::AcceptedFormats(vec![
            SourceFormatV1::Glb,
        ]));
        let descriptor = EngineSettingDescriptorV2::new(
            EngineSettingIdV2::LoadMeshes,
            EngineSettingScopeV1::Document,
            EngineSettingDomainV2::Token,
            vec![SourceFormatV1::Glb],
            Some(EngineSettingValueV2::Token("nonempty".into())),
        )
        .unwrap();
        let source = EnginePrimarySourceV2::new(
            "bevy-doc",
            "0.19",
            "https://example.invalid/bevy",
            "2026-08-25",
            vec![EngineFactIdV2::AcceptedInputs],
            vec![EngineSettingIdV2::LoadMeshes],
        )
        .unwrap();
        ResolvedEngineProfileV2::new(
            EngineProfileSelectionV1::new("bevy", 2, "0.19", "bevy_gltf").unwrap(),
            "urn:animsmith:engine-profile:bevy:0.19:2",
            facts,
            vec![descriptor],
            vec![source],
        )
        .unwrap()
    }

    #[test]
    fn v3_value_origin_is_identity_bearing_and_applicable_rows_are_required() {
        let profile = profile_v2_for_origin_tests();
        let row = |origin| {
            EngineSettingRowV3::new(
                EngineSettingIdV2::LoadMeshes,
                EngineSettingValueV2::Token("nonempty".into()),
                origin,
            )
        };
        let default = ResolvedEngineSettingsV3::new(
            &profile,
            SourceFormatV1::Glb,
            vec![row(EngineSettingValueOriginV3::ProfileDefault)],
            vec![],
            ResolvedEngineSettingsCoverageV2::complete(),
            ResolvedEngineSettingsWorkV2::new(0, 0, 0),
        )
        .unwrap();
        let explicit = ResolvedEngineSettingsV3::new(
            &profile,
            SourceFormatV1::Glb,
            vec![row(EngineSettingValueOriginV3::ExplicitConfig)],
            vec![],
            ResolvedEngineSettingsCoverageV2::complete(),
            ResolvedEngineSettingsWorkV2::new(0, 0, 0),
        )
        .unwrap();
        assert_ne!(default.settings_identity(), explicit.settings_identity());
        assert!(matches!(
            ResolvedEngineSettingsV3::new(
                &profile,
                SourceFormatV1::Glb,
                vec![],
                vec![],
                ResolvedEngineSettingsCoverageV2::complete(),
                ResolvedEngineSettingsWorkV2::new(0, 0, 0),
            ),
            Err(EngineContractError::MissingApplicableV2Setting {
                setting: EngineSettingIdV2::LoadMeshes,
                ..
            })
        ));
    }

    #[test]
    fn v2_profile_and_v3_settings_readers_stop_at_each_nested_n_plus_one() {
        let profile = profile_v2_for_origin_tests();
        let mut profile_wire = serde_json::to_value(&profile).unwrap();
        for field in ["facts", "setting_descriptors", "primary_sources"] {
            let element = profile_wire[field][0].clone();
            profile_wire[field] = vec![element; ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS].into();
            profile_wire[field]
                .as_array_mut()
                .unwrap()
                .push(serde_json::Value::Null);
            assert!(
                serde_json::from_value::<ResolvedEngineProfileV2>(profile_wire.clone()).is_err()
            );
            profile_wire = serde_json::to_value(&profile).unwrap();
        }

        let settings = ResolvedEngineSettingsV3::new(
            &profile,
            SourceFormatV1::Glb,
            vec![EngineSettingRowV3::new(
                EngineSettingIdV2::LoadMeshes,
                EngineSettingValueV2::Token("nonempty".into()),
                EngineSettingValueOriginV3::ProfileDefault,
            )],
            vec![],
            ResolvedEngineSettingsCoverageV2::complete(),
            ResolvedEngineSettingsWorkV2::new(0, 0, 0),
        )
        .unwrap();
        let mut settings_wire = serde_json::to_value(&settings).unwrap();
        let row = settings_wire["document_settings"][0].clone();
        settings_wire["document_settings"] =
            vec![row; ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS].into();
        settings_wire["document_settings"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::Value::Null);
        assert!(serde_json::from_value::<ResolvedEngineSettingsV3>(settings_wire).is_err());

        let mut nested = serde_json::to_value(settings).unwrap();
        let mut rows = vec![
            serde_json::json!({
                "id": "load_meshes",
                "value": {"token": "nonempty"},
                "value_origin": "profile_default"
            });
            ENGINE_CONTRACT_V1_MAX_COLLECTION_ROWS
        ];
        rows.push(serde_json::Value::Null);
        nested["clips"] = serde_json::json!([{
            "clip_ordinal": 0,
            "clip_name": "clip",
            "settings": rows
        }]);
        assert!(serde_json::from_value::<ResolvedEngineSettingsV3>(nested).is_err());
    }
}
