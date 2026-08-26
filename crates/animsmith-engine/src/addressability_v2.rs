//! Rich, bounded glTF addressability for the exact Bevy 0.19.0 loader.
//!
//! This module is a successor to the animation-only V1 report.  It keeps that
//! inventory intact and adds a separately versioned Bevy rule bundle for scene,
//! skin, named-map, and animation-target projections.  None of the projections
//! certify that Bevy loaded, spawned, or played the source asset at runtime.

use crate::{
    BevyAnimationAssetLabelV1, GltfAnimationAddressabilityInventoryV1, GltfAnimationObservationV1,
    ResolvedProfileSettingsV2, ResolvedSettingOriginV2, SettingIdV2, SettingValueV2,
};
use animsmith_core::{
    Applicability, Check, CheckCtx, CheckEvaluation, CheckOutput, CheckSelection,
    ConfigurationState, EnginePredictionBasisV4, EnginePredictionFacetStateV1,
    EnginePredictionFacetV4, EnginePredictionV4, EngineSettingIdV2, EngineSettingValueOriginV3,
    EngineSettingValueV2, EvaluationScope, EvaluationScopeCode, EvaluationState, InputIdentity,
    LoadedSource, PREDICTION_V1_MAX_FACETS_PER_FILE, PredictionBasisReferenceV1,
    PredictionBasisReferenceV2, PredictionBasisReferenceV4, PredictionProvenanceV4,
    PredictionUnavailableReasonV2, RawGltfAddressabilityInventoryV1,
    RawGltfDefaultSceneObservationV1, RawSourceBasisReferenceV1, RawSourceDomainV1,
    RawSourceFieldIdV1, RawSourceKeyV1, ResolvedSettingLocationV1, SelectionState, ToolInfo,
    evaluate_checks,
};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::Read;

/// Version of the rich standalone glTF addressability envelope.
pub const GLTF_ADDRESSABILITY_V2_SCHEMA_VERSION: u32 = 2;
/// Immutable identity of the rich standalone glTF addressability contract.
pub const GLTF_ADDRESSABILITY_V2_ID: &str = "urn:animsmith:schema:gltf-addressability:2";
/// Maximum serialized bytes accepted by the rich report reader.
pub const GLTF_ADDRESSABILITY_V2_MAX_REPORT_BYTES: u64 = 256 * 1024 * 1024;

/// Maximum rows retained independently for each rich addressability domain.
pub const GLTF_ADDRESSABILITY_V2_MAX_DOMAIN_ROWS: usize = 4_096;
/// Maximum aggregate structural references retained by the new rich
/// projections (the two embedded sealed inventories enforce their own bounds).
pub const GLTF_ADDRESSABILITY_V2_MAX_STRUCTURAL_REFERENCES: usize = 65_536;
/// Maximum completed check scopes implied by all bounded V2 row domains.
pub const GLTF_ADDRESSABILITY_V2_MAX_EVALUATED_SCOPES: usize =
    5 * GLTF_ADDRESSABILITY_V2_MAX_DOMAIN_ROWS + 5;
/// Maximum UTF-8 bytes retained by one source name or path segment.
pub const GLTF_ADDRESSABILITY_V2_MAX_SEGMENT_BYTES: usize = 1_024;
/// Maximum UTF-8 bytes retained by one slash-delimited target path.
pub const GLTF_ADDRESSABILITY_V2_MAX_PATH_BYTES: usize = 4_096;
/// Maximum segments retained by one target path.
pub const GLTF_ADDRESSABILITY_V2_MAX_PATH_SEGMENTS: usize = 256;
/// Maximum aggregate text retained by the new rich projections (excluding the
/// two embedded sealed inventories, which enforce their own text bounds).
pub const GLTF_ADDRESSABILITY_V2_MAX_TOTAL_TEXT_BYTES: usize = 1024 * 1024;

const BEVY_TAG: &str = "v0.19.0";
const BEVY_COMMIT: &str = "c6f634ca9f406d68ba5109d921247b654cb42c10";
const BEVY_GLTF_CRATE: &str = "bevy_gltf 0.19.0";
const GLTF_CRATE: &str = "gltf 1.4.1";
const BEVY_RULE_BUNDLE_ID: &str = "urn:animsmith:bevy-gltf-addressability-rules:1";
const BEVY_PROFILE_REVISION: u32 = 3;
const FACET_BUDGET_SCOPE: &str = "engine-addressability:facet-budget";
const ANIMATION_TARGET_NAMESPACE: [u8; 16] = [
    0x31, 0x79, 0xf5, 0x19, 0xd9, 0x27, 0x4f, 0xf2, 0xb5, 0x96, 0x6f, 0xd0, 0x77, 0x02, 0x39, 0x11,
];

/// Explicit target pointer width used by Bevy's target-id preimage.
///
/// Bevy hashes every path segment's byte length using
/// `usize::to_le_bytes()`.  The width is therefore an input to the exact
/// prediction and is never inferred from the host running AnimSmith.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetPointerWidth {
    /// Four-byte little-endian segment lengths.
    Bits32,
    /// Eight-byte little-endian segment lengths.
    Bits64,
}

impl TargetPointerWidth {
    const fn bytes(self) -> usize {
        match self {
            Self::Bits32 => 4,
            Self::Bits64 => 8,
        }
    }
}

/// Named-map duplicate policy in the pinned Bevy loader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BevyNamedMapDuplicatePolicyV1 {
    /// Later insertions replace an earlier value with the same exact name.
    LastWriteWins,
}

/// Immutable primary-source authority for one Bevy addressability rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BevyAddressabilityRuleSourceV1 {
    id: String,
    url: String,
}

impl BevyAddressabilityRuleSourceV1 {
    /// Stable source identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Commit-pinned primary-source URL.
    pub fn url(&self) -> &str {
        &self.url
    }
}

/// Separately versioned exact Bevy glTF addressability authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BevyGltfAddressabilityRulesV1 {
    schema: String,
    profile_revision: u32,
    bevy_tag: String,
    bevy_commit: String,
    bevy_gltf_crate: String,
    locked_gltf_crate: String,
    bevy_animation_feature_required_for_targets: bool,
    load_animations_required_for_targets: bool,
    target_pointer_width: Option<TargetPointerWidth>,
    named_scene_policy: BevyNamedMapDuplicatePolicyV1,
    named_animation_policy: BevyNamedMapDuplicatePolicyV1,
    named_skin_policy: BevyNamedMapDuplicatePolicyV1,
    sources: Vec<BevyAddressabilityRuleSourceV1>,
}

impl BevyGltfAddressabilityRulesV1 {
    /// Materialize the frozen revision-1 rules for the exact revision-3 Bevy
    /// profile and an optional explicit target width.
    pub fn frozen(target_pointer_width: Option<TargetPointerWidth>) -> Self {
        let pinned =
            |path: &str| format!("https://github.com/bevyengine/bevy/blob/{BEVY_COMMIT}/{path}");
        let mut sources = vec![
            BevyAddressabilityRuleSourceV1 {
                id: "bevy-animation-target-id-0.19.0-c6f634ca".into(),
                url: pinned("crates/bevy_animation/src/lib.rs"),
            },
            BevyAddressabilityRuleSourceV1 {
                id: "bevy-gltf-labels-0.19.0-c6f634ca".into(),
                url: pinned("crates/bevy_gltf/src/label.rs"),
            },
            BevyAddressabilityRuleSourceV1 {
                id: "bevy-gltf-loader-0.19.0-c6f634ca".into(),
                url: pinned("crates/bevy_gltf/src/loader/mod.rs"),
            },
            BevyAddressabilityRuleSourceV1 {
                id: "bevy-gltf-node-path-0.19.0-c6f634ca".into(),
                url: pinned("crates/bevy_gltf/src/loader/gltf_ext/scene.rs"),
            },
            BevyAddressabilityRuleSourceV1 {
                id: "bevy-gltf-manifest-0.19.0-c6f634ca".into(),
                url: pinned("crates/bevy_gltf/Cargo.toml"),
            },
            BevyAddressabilityRuleSourceV1 {
                id: "bevy-lockfile-0.19.0-c6f634ca".into(),
                url: pinned("Cargo.lock"),
            },
        ];
        sources.sort_by(|left, right| left.id.cmp(&right.id));
        Self {
            schema: BEVY_RULE_BUNDLE_ID.into(),
            profile_revision: BEVY_PROFILE_REVISION,
            bevy_tag: BEVY_TAG.into(),
            bevy_commit: BEVY_COMMIT.into(),
            bevy_gltf_crate: BEVY_GLTF_CRATE.into(),
            locked_gltf_crate: GLTF_CRATE.into(),
            bevy_animation_feature_required_for_targets: true,
            load_animations_required_for_targets: true,
            target_pointer_width,
            named_scene_policy: BevyNamedMapDuplicatePolicyV1::LastWriteWins,
            named_animation_policy: BevyNamedMapDuplicatePolicyV1::LastWriteWins,
            named_skin_policy: BevyNamedMapDuplicatePolicyV1::LastWriteWins,
            sources,
        }
    }

    /// Immutable rule-bundle schema identity.
    pub fn contract_id(&self) -> &str {
        &self.schema
    }

    /// Exact profile revision this separate rule bundle may accompany.
    pub const fn profile_revision(&self) -> u32 {
        self.profile_revision
    }

    /// Explicit target width, absent when exact target IDs are unavailable.
    pub const fn target_pointer_width(&self) -> Option<TargetPointerWidth> {
        self.target_pointer_width
    }

    /// Commit-pinned primary sources in canonical identifier order.
    pub fn sources(&self) -> &[BevyAddressabilityRuleSourceV1] {
        &self.sources
    }

    fn validate(&self) -> bool {
        self == &Self::frozen(self.target_pointer_width)
    }
}

/// Origin of one exact setting consumed by the rich Bevy projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GltfAddressabilitySettingOriginV2 {
    /// The caller explicitly selected the value.
    ExplicitConfig,
    /// The frozen profile supplied its verified default.
    ProfileDefault,
}

/// Exact revision-3 settings consumed by the addressability rule bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BevyGltfAddressabilitySettingsV2 {
    bevy_animation_feature: bool,
    bevy_animation_feature_origin: GltfAddressabilitySettingOriginV2,
    load_animations: bool,
    load_animations_origin: GltfAddressabilitySettingOriginV2,
}

impl BevyGltfAddressabilitySettingsV2 {
    fn from_resolved(
        resolved: &ResolvedProfileSettingsV2,
    ) -> Result<Self, GltfAddressabilityV2Error> {
        let selection = resolved.profile().selection();
        if selection.family() != "bevy"
            || selection.profile_revision() != BEVY_PROFILE_REVISION
            || selection.engine_version() != "0.19.0"
            || selection.importer() != "gltf-asset-loader"
        {
            return Err(GltfAddressabilityV2Error::InvalidBevyProfile);
        }
        let boolean = |id| {
            let setting = resolved
                .document_settings()
                .get(&id)
                .ok_or(GltfAddressabilityV2Error::MissingRequiredSetting)?;
            let SettingValueV2::Boolean(value) = setting.value() else {
                return Err(GltfAddressabilityV2Error::MissingRequiredSetting);
            };
            Ok((*value, setting_origin(setting.origin())))
        };
        let (bevy_animation_feature, bevy_animation_feature_origin) =
            boolean(SettingIdV2::BevyAnimationFeature)?;
        let (load_animations, load_animations_origin) = boolean(SettingIdV2::LoadAnimations)?;
        Ok(Self {
            bevy_animation_feature,
            bevy_animation_feature_origin,
            load_animations,
            load_animations_origin,
        })
    }

    /// Whether Bevy's compile-time animation feature is present.
    pub const fn bevy_animation_feature(&self) -> bool {
        self.bevy_animation_feature
    }

    /// Whether the feature value was explicit or supplied by the profile.
    pub const fn bevy_animation_feature_origin(&self) -> GltfAddressabilitySettingOriginV2 {
        self.bevy_animation_feature_origin
    }

    /// Exact loader animation toggle.
    pub const fn load_animations(&self) -> bool {
        self.load_animations
    }

    /// Whether the loader toggle was explicit or supplied by the profile.
    pub const fn load_animations_origin(&self) -> GltfAddressabilitySettingOriginV2 {
        self.load_animations_origin
    }
}

const fn setting_origin(value: ResolvedSettingOriginV2) -> GltfAddressabilitySettingOriginV2 {
    match value {
        ResolvedSettingOriginV2::ExplicitConfig => {
            GltfAddressabilitySettingOriginV2::ExplicitConfig
        }
        ResolvedSettingOriginV2::ProfileDefault => {
            GltfAddressabilitySettingOriginV2::ProfileDefault
        }
    }
}

/// Reproduce Bevy 0.19.0's `AnimationTargetId::from_names` exactly.
///
/// The returned string is the lower-case hyphenated UUID spelling Bevy emits.
/// Segment byte lengths are encoded at the explicitly supplied target width.
///
/// # Errors
///
/// Returns [`BevyAnimationTargetIdError`] when a segment or complete path is
/// outside the V2 bounds, or a segment length cannot fit the chosen width.
pub fn bevy_animation_target_id_v1<'a>(
    segments: impl IntoIterator<Item = &'a str>,
    target_pointer_width: TargetPointerWidth,
) -> Result<String, BevyAnimationTargetIdError> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&ANIMATION_TARGET_NAMESPACE);
    let mut segment_count = 0usize;
    let mut path_bytes = 0usize;
    for segment in segments {
        segment_count = segment_count
            .checked_add(1)
            .ok_or(BevyAnimationTargetIdError::ArithmeticOverflow)?;
        if segment_count > GLTF_ADDRESSABILITY_V2_MAX_PATH_SEGMENTS {
            return Err(BevyAnimationTargetIdError::TooManySegments {
                found: segment_count,
                limit: GLTF_ADDRESSABILITY_V2_MAX_PATH_SEGMENTS,
            });
        }
        if segment.len() > GLTF_ADDRESSABILITY_V2_MAX_SEGMENT_BYTES {
            return Err(BevyAnimationTargetIdError::SegmentTooLong {
                found: segment.len(),
                limit: GLTF_ADDRESSABILITY_V2_MAX_SEGMENT_BYTES,
            });
        }
        path_bytes = path_bytes
            .checked_add(segment.len())
            .and_then(|value| value.checked_add(usize::from(segment_count > 1)))
            .ok_or(BevyAnimationTargetIdError::ArithmeticOverflow)?;
        if path_bytes > GLTF_ADDRESSABILITY_V2_MAX_PATH_BYTES {
            return Err(BevyAnimationTargetIdError::PathTooLong {
                found: path_bytes,
                limit: GLTF_ADDRESSABILITY_V2_MAX_PATH_BYTES,
            });
        }
        match target_pointer_width.bytes() {
            4 => hasher.update(
                &u32::try_from(segment.len())
                    .map_err(|_| BevyAnimationTargetIdError::SegmentLengthOverflow)?
                    .to_le_bytes(),
            ),
            8 => hasher.update(
                &u64::try_from(segment.len())
                    .map_err(|_| BevyAnimationTargetIdError::SegmentLengthOverflow)?
                    .to_le_bytes(),
            ),
            _ => unreachable!("closed target pointer width"),
        };
        hasher.update(segment.as_bytes());
    }
    let mut uuid = [0u8; 16];
    uuid.copy_from_slice(&hasher.finalize().as_bytes()[..16]);
    // `uuid::Builder::from_sha1_bytes`: retain the supplied bytes, then set
    // RFC 4122 variant and version 5 bits.  Bevy intentionally uses this UUID
    // builder even though the input bytes are the BLAKE3 prefix.
    uuid[6] = (uuid[6] & 0x0f) | 0x50;
    uuid[8] = (uuid[8] & 0x3f) | 0x80;
    let mut value = String::with_capacity(36);
    for (index, byte) in uuid.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            value.push('-');
        }
        write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(value)
}

/// A bounded Bevy animation-target UUID could not be reproduced.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum BevyAnimationTargetIdError {
    /// A path has more than 256 segments.
    #[error("target path has {found} segments; limit is {limit}")]
    TooManySegments {
        /// Observed segment count.
        found: usize,
        /// Inclusive segment limit.
        limit: usize,
    },
    /// One segment exceeds 1,024 UTF-8 bytes.
    #[error("target path segment has {found} bytes; limit is {limit}")]
    SegmentTooLong {
        /// Observed byte count.
        found: usize,
        /// Inclusive byte limit.
        limit: usize,
    },
    /// The complete slash-delimited path exceeds 4,096 bytes.
    #[error("target path has {found} bytes; limit is {limit}")]
    PathTooLong {
        /// Observed byte count.
        found: usize,
        /// Inclusive byte limit.
        limit: usize,
    },
    /// A segment length cannot be represented at the selected target width.
    #[error("target path segment length does not fit the selected pointer width")]
    SegmentLengthOverflow,
    /// Checked bound arithmetic overflowed.
    #[error("target path bound arithmetic overflowed")]
    ArithmeticOverflow,
}

/// Stable reason one exact rich Bevy projection is required-unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GltfAddressabilityUnavailableReasonV2 {
    /// One or more raw source domains are not exhaustive.
    RawSourceIncomplete,
    /// The same-load dependency closure is not complete.
    DependencyClosureIncomplete,
    /// The source target has no scene-root path.
    UnreachableTarget,
    /// A target has multiple distinct all-scene path candidates.
    MultipleCandidatePaths,
    /// Different source target nodes have the same full Bevy name path.
    DuplicateFullPath,
    /// Different full paths reproduce the same Bevy target UUID.
    TargetIdCollision,
    /// The exact target pointer width was not declared.
    TargetPointerWidthMissing,
    /// Bevy was not declared with the `bevy_animation` feature enabled.
    BevyAnimationFeatureDisabled,
    /// Bevy's glTF `load_animations` setting is disabled.
    LoadAnimationsDisabled,
    /// A bounded path cannot be represented by this contract.
    PathBoundsExceeded,
    /// The named map cannot be proven exhaustive.
    NamedMapIncomplete,
    /// The bounded target domain retained only a canonical prefix.
    TargetDomainTruncated,
    /// A new rich projection exceeded its independently declared bound.
    ProjectionBoundsExceeded,
}

/// Exact projection state for one rich addressability value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum GltfAddressabilityProjectionV2<T> {
    /// An exact value is available.
    Available {
        /// Exact projected value.
        value: T,
    },
    /// Complete evidence proves that this conditional route/label is absent.
    ProvenAbsent,
    /// Exact prediction was required but one or more prerequisites were absent.
    RequiredUnavailable {
        /// Canonical typed reasons.
        reasons: Vec<GltfAddressabilityUnavailableReasonV2>,
    },
}

impl<T> GltfAddressabilityProjectionV2<T> {
    fn unavailable(mut reasons: Vec<GltfAddressabilityUnavailableReasonV2>) -> Self {
        reasons.sort();
        reasons.dedup();
        debug_assert!(!reasons.is_empty());
        Self::RequiredUnavailable { reasons }
    }

    fn is_unavailable(&self) -> bool {
        matches!(self, Self::RequiredUnavailable { .. })
    }
}

/// One exact typed Bevy scene label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GltfAddressabilitySceneV2 {
    source_scene_index: u64,
    label: String,
}

impl GltfAddressabilitySceneV2 {
    /// Exact source scene-array index.
    pub const fn source_scene_index(&self) -> u64 {
        self.source_scene_index
    }

    /// Exact typed label `Scene{i}`.
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// One source skin's distinct conditional skin and eager inverse-bind labels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GltfAddressabilitySkinV2 {
    source_skin_index: u64,
    explicit_skeleton_root_node_index: Option<u64>,
    skin_label: GltfAddressabilityProjectionV2<String>,
    inverse_bind_matrices_label: String,
}

impl GltfAddressabilitySkinV2 {
    /// Exact source skin-array index.
    pub const fn source_skin_index(&self) -> u64 {
        self.source_skin_index
    }

    /// Exact authored `skin.skeleton`, preserved without a Bevy inferred-root claim.
    pub const fn explicit_skeleton_root_node_index(&self) -> Option<u64> {
        self.explicit_skeleton_root_node_index
    }

    /// Conditional `Skin{i}` label projection.
    pub const fn skin_label(&self) -> &GltfAddressabilityProjectionV2<String> {
        &self.skin_label
    }

    /// Eager `Skin{i}/InverseBindMatrices` label, including identity fallback.
    pub fn inverse_bind_matrices_label(&self) -> &str {
        &self.inverse_bind_matrices_label
    }
}

/// One Bevy named-map domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GltfAddressabilityNamedMapKindV2 {
    /// `Gltf.named_scenes`.
    Scene,
    /// `Gltf.named_animations`.
    Animation,
    /// `Gltf.named_skins`.
    Skin,
}

/// One exact named-map winner, separate from typed index labels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GltfAddressabilityNamedMapWinnerV2 {
    name: String,
    source_index: u64,
    typed_label: String,
}

impl GltfAddressabilityNamedMapWinnerV2 {
    /// Exact authored map key.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Exact source-array identity selected by the winner.
    pub const fn source_index(&self) -> u64 {
        self.source_index
    }

    /// Exact typed label of the selected source row.
    pub fn typed_label(&self) -> &str {
        &self.typed_label
    }
}

/// One independently coverage-qualified named-map projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GltfAddressabilityNamedMapV2 {
    kind: GltfAddressabilityNamedMapKindV2,
    duplicate_policy: BevyNamedMapDuplicatePolicyV1,
    winners: GltfAddressabilityProjectionV2<Vec<GltfAddressabilityNamedMapWinnerV2>>,
}

impl GltfAddressabilityNamedMapV2 {
    /// Runtime named-map domain represented by this row.
    pub const fn kind(&self) -> GltfAddressabilityNamedMapKindV2 {
        self.kind
    }

    /// Exact duplicate-name winner policy used for this map.
    pub const fn duplicate_policy(&self) -> BevyNamedMapDuplicatePolicyV1 {
        self.duplicate_policy
    }

    /// Exact winners, proven empty state, or typed unavailability.
    pub const fn winners(
        &self,
    ) -> &GltfAddressabilityProjectionV2<Vec<GltfAddressabilityNamedMapWinnerV2>> {
        &self.winners
    }
}

/// One source animation channel contributing to a unique target node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GltfAddressabilityTargetChannelV2 {
    source_animation_index: u64,
    source_channel_index: u64,
}

impl GltfAddressabilityTargetChannelV2 {
    /// Exact source animation-array index.
    pub const fn source_animation_index(&self) -> u64 {
        self.source_animation_index
    }

    /// Exact source channel index within that animation.
    pub const fn source_channel_index(&self) -> u64 {
        self.source_channel_index
    }
}

/// Exact Bevy target path and reproduced UUID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GltfAddressabilityTargetValueV2 {
    segments: Vec<String>,
    path: String,
    uuid: String,
}

impl GltfAddressabilityTargetValueV2 {
    /// Authored-or-fallback path segments, excluding the scene world root.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Slash-delimited display path.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Exact lower-case Bevy `AnimationTargetId` UUID.
    pub fn uuid(&self) -> &str {
        &self.uuid
    }
}

/// One unique source animation target node and all contributing channels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GltfAddressabilityTargetV2 {
    source_node_index: u64,
    contributing_channels: Vec<GltfAddressabilityTargetChannelV2>,
    projection: GltfAddressabilityProjectionV2<GltfAddressabilityTargetValueV2>,
}

impl GltfAddressabilityTargetV2 {
    /// Exact source node-array identity.
    pub const fn source_node_index(&self) -> u64 {
        self.source_node_index
    }

    /// Canonical source animation/channel contributors.
    pub fn contributing_channels(&self) -> &[GltfAddressabilityTargetChannelV2] {
        &self.contributing_channels
    }

    /// Exact path/UUID or typed required-unavailable evidence.
    pub const fn projection(
        &self,
    ) -> &GltfAddressabilityProjectionV2<GltfAddressabilityTargetValueV2> {
        &self.projection
    }
}

/// Structured rich projections paired with the one engine-addressability check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BevyGltfAddressabilityProjectionV2 {
    scenes: Vec<GltfAddressabilitySceneV2>,
    default_scene_route: GltfAddressabilityProjectionV2<String>,
    skins: Vec<GltfAddressabilitySkinV2>,
    named_maps: Vec<GltfAddressabilityNamedMapV2>,
    target_coverage: GltfAddressabilityProjectionV2<()>,
    targets: Vec<GltfAddressabilityTargetV2>,
}

impl BevyGltfAddressabilityProjectionV2 {
    /// Project rich Bevy values from the immutable raw sidecar and unchanged
    /// animation inventory.
    pub fn from_inventories(
        raw: &RawGltfAddressabilityInventoryV1,
        animations: &GltfAnimationAddressabilityInventoryV1,
        bevy_animation_enabled: bool,
        load_animations: bool,
        pointer_width: Option<TargetPointerWidth>,
    ) -> Result<Self, GltfAddressabilityV2Error> {
        validate_inventory_binding(raw, animations)?;
        let scenes = raw
            .scenes()
            .iter()
            .map(|scene| GltfAddressabilitySceneV2 {
                source_scene_index: scene.source_scene_index(),
                label: format!("Scene{}", scene.source_scene_index()),
            })
            .collect();
        let default_scene_route = match raw.default_scene() {
            RawGltfDefaultSceneObservationV1::Absent => {
                GltfAddressabilityProjectionV2::ProvenAbsent
            }
            RawGltfDefaultSceneObservationV1::Selected { source_scene_index }
                if raw
                    .scenes()
                    .get(source_scene_index as usize)
                    .is_some_and(|scene| scene.source_scene_index() == source_scene_index) =>
            {
                GltfAddressabilityProjectionV2::Available {
                    value: format!("Scene{source_scene_index}"),
                }
            }
            RawGltfDefaultSceneObservationV1::Selected { .. }
            | RawGltfDefaultSceneObservationV1::Unavailable { .. } => {
                GltfAddressabilityProjectionV2::unavailable(vec![
                    GltfAddressabilityUnavailableReasonV2::RawSourceIncomplete,
                ])
            }
        };

        let attached = raw
            .attachments()
            .iter()
            .map(|row| row.source_skin_index())
            .collect::<BTreeSet<_>>();
        let skins = raw
            .skins()
            .iter()
            .map(|skin| {
                let source_skin_index = skin.source_skin_index();
                let skin_label = if attached.contains(&source_skin_index) {
                    GltfAddressabilityProjectionV2::Available {
                        value: format!("Skin{source_skin_index}"),
                    }
                } else if raw.attachment_coverage().is_complete() {
                    GltfAddressabilityProjectionV2::ProvenAbsent
                } else {
                    GltfAddressabilityProjectionV2::unavailable(vec![
                        GltfAddressabilityUnavailableReasonV2::RawSourceIncomplete,
                    ])
                };
                GltfAddressabilitySkinV2 {
                    source_skin_index,
                    explicit_skeleton_root_node_index: skin.skeleton_root_node_index(),
                    skin_label,
                    inverse_bind_matrices_label: format!(
                        "Skin{source_skin_index}/InverseBindMatrices"
                    ),
                }
            })
            .collect();

        let named_maps = project_named_maps(raw, animations);
        let (target_coverage, targets) = project_targets(
            raw,
            animations,
            bevy_animation_enabled,
            load_animations,
            pointer_width,
        )?;
        let mut projection = Self {
            scenes,
            default_scene_route,
            skins,
            named_maps,
            target_coverage,
            targets,
        };
        normalize_projection_bounds(&mut projection);
        Ok(projection)
    }

    /// Exact typed scene-label rows.
    pub fn scenes(&self) -> &[GltfAddressabilitySceneV2] {
        &self.scenes
    }

    /// Optional route to one existing `Scene{i}`.
    pub const fn default_scene_route(&self) -> &GltfAddressabilityProjectionV2<String> {
        &self.default_scene_route
    }

    /// Exact source-skin rows with distinct conditional/eager labels.
    pub fn skins(&self) -> &[GltfAddressabilitySkinV2] {
        &self.skins
    }

    /// Scene, animation, and skin named-map projections.
    pub fn named_maps(&self) -> &[GltfAddressabilityNamedMapV2] {
        &self.named_maps
    }

    /// Unique source animation target-node projections.
    pub fn targets(&self) -> &[GltfAddressabilityTargetV2] {
        &self.targets
    }

    /// Whether the unique-target collection is exhaustive.
    pub const fn target_coverage(&self) -> &GltfAddressabilityProjectionV2<()> {
        &self.target_coverage
    }

    /// Whether any rich prediction is required-unavailable.
    pub fn has_required_unavailable(&self) -> bool {
        self.default_scene_route.is_unavailable()
            || self.skins.iter().any(|row| row.skin_label.is_unavailable())
            || self
                .named_maps
                .iter()
                .any(|row| row.winners.is_unavailable())
            || self
                .targets
                .iter()
                .any(|row| row.projection.is_unavailable())
            || self.target_coverage.is_unavailable()
    }
}

/// Rich engine-neutral inventory adapter that reuses the immutable V1
/// animation inventory beside the raw scene/node/skin sidecar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GltfAddressabilityInventoryV2 {
    schema: String,
    raw: RawGltfAddressabilityInventoryV1,
    animations: GltfAnimationAddressabilityInventoryV1,
}

impl GltfAddressabilityInventoryV2 {
    /// Bind the two immutable same-load inventories.
    pub fn new(
        raw: RawGltfAddressabilityInventoryV1,
        animations: GltfAnimationAddressabilityInventoryV1,
    ) -> Result<Self, GltfAddressabilityV2Error> {
        validate_inventory_binding(&raw, &animations)?;
        Ok(Self {
            schema: GLTF_ADDRESSABILITY_V2_ID.into(),
            raw,
            animations,
        })
    }

    /// Immutable rich inventory schema identity.
    pub fn contract_id(&self) -> &str {
        &self.schema
    }

    /// Raw scene/node/skin/path sidecar.
    pub const fn raw(&self) -> &RawGltfAddressabilityInventoryV1 {
        &self.raw
    }

    /// Unchanged V1 animation inventory.
    pub const fn animations(&self) -> &GltfAnimationAddressabilityInventoryV1 {
        &self.animations
    }

    fn validate(&self) -> Result<(), GltfAddressabilityV2Error> {
        if self.schema != GLTF_ADDRESSABILITY_V2_ID {
            return Err(GltfAddressabilityV2Error::InvalidSchema);
        }
        validate_inventory_binding(&self.raw, &self.animations)
    }
}

/// Exact Bevy adapter paired with one real `engine-addressability` evaluation.
#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GltfAddressabilityBevyAdapterV2 {
    rules: BevyGltfAddressabilityRulesV1,
    settings: BevyGltfAddressabilitySettingsV2,
    prediction_provenance: PredictionProvenanceV4,
    check: CheckEvaluation,
    projection: BevyGltfAddressabilityProjectionV2,
}

impl GltfAddressabilityBevyAdapterV2 {
    /// Frozen separately versioned rule authority.
    pub const fn rules(&self) -> &BevyGltfAddressabilityRulesV1 {
        &self.rules
    }

    /// Exact consumed revision-3 settings and origins.
    pub const fn settings(&self) -> &BevyGltfAddressabilitySettingsV2 {
        &self.settings
    }

    /// Same-load V4 prediction provenance.
    pub const fn prediction_provenance(&self) -> &PredictionProvenanceV4 {
        &self.prediction_provenance
    }

    /// The one shared check lifecycle evaluation.
    pub const fn check(&self) -> &CheckEvaluation {
        &self.check
    }

    /// Structured projections validated against the check facets.
    pub const fn projection(&self) -> &BevyGltfAddressabilityProjectionV2 {
        &self.projection
    }
}

/// Standalone producer envelope for rich glTF addressability.
#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GltfAddressabilityV2 {
    schema_version: u32,
    schema: &'static str,
    tool: ToolInfo,
    command: &'static str,
    input: InputIdentity,
    inventory: GltfAddressabilityInventoryV2,
    bevy: Option<GltfAddressabilityBevyAdapterV2>,
}

impl GltfAddressabilityV2 {
    /// Construct one validated rich report.
    pub fn new(
        tool: ToolInfo,
        raw: RawGltfAddressabilityInventoryV1,
        animations: GltfAnimationAddressabilityInventoryV1,
        bevy: Option<GltfAddressabilityBevyAdapterV2>,
    ) -> Result<Self, GltfAddressabilityV2Error> {
        let inventory = GltfAddressabilityInventoryV2::new(raw, animations)?;
        if let Some(adapter) = &bevy {
            validate_adapter(adapter, &inventory)?;
        }
        Ok(Self {
            schema_version: GLTF_ADDRESSABILITY_V2_SCHEMA_VERSION,
            schema: GLTF_ADDRESSABILITY_V2_ID,
            tool,
            command: crate::GLTF_ANIMATION_ADDRESSABILITY_COMMAND,
            input: inventory.raw.primary_input().clone(),
            inventory,
            bevy,
        })
    }

    /// Exact primary input identity.
    pub const fn input(&self) -> &InputIdentity {
        &self.input
    }

    /// Rich engine-neutral inventory adapter.
    pub const fn inventory(&self) -> &GltfAddressabilityInventoryV2 {
        &self.inventory
    }

    /// Exact richer Bevy adapter when revision 3 was selected.
    pub const fn bevy(&self) -> Option<&GltfAddressabilityBevyAdapterV2> {
        self.bevy.as_ref()
    }

    /// Whether the real embedded check carries required-unavailable work.
    pub fn has_required_prediction_unavailable(&self) -> bool {
        self.bevy
            .as_ref()
            .is_some_and(|adapter| adapter.check.has_required_prediction_unavailable())
    }

    /// Strictly read and cross-validate one V2 report through the 256 MiB cap.
    pub fn read_from(
        reader: impl Read,
    ) -> Result<GltfAddressabilityReadbackV2, GltfAddressabilityReadErrorV2> {
        let mut bounded = reader.take(GLTF_ADDRESSABILITY_V2_MAX_REPORT_BYTES + 1);
        let mut bytes = Vec::new();
        bounded
            .read_to_end(&mut bytes)
            .map_err(|source| GltfAddressabilityReadErrorV2::Io { source })?;
        if bytes.len() as u64 > GLTF_ADDRESSABILITY_V2_MAX_REPORT_BYTES {
            return Err(GltfAddressabilityReadErrorV2::ReportTooLarge {
                limit: GLTF_ADDRESSABILITY_V2_MAX_REPORT_BYTES,
            });
        }
        let wire: GltfAddressabilityWireV2 = serde_json::from_slice(&bytes)
            .map_err(|source| GltfAddressabilityReadErrorV2::InvalidJson { source })?;
        GltfAddressabilityReadbackV2::from_wire(wire)
            .map_err(GltfAddressabilityReadErrorV2::Contract)
    }
}

/// Build the optional exact revision-3 Bevy adapter through one check run.
pub fn build_bevy_addressability_adapter_v2(
    source: &LoadedSource,
    raw: &RawGltfAddressabilityInventoryV1,
    animations: &GltfAnimationAddressabilityInventoryV1,
    resolved: &ResolvedProfileSettingsV2,
    prediction_provenance: PredictionProvenanceV4,
    pointer_width: Option<TargetPointerWidth>,
    ctx: &CheckCtx<'_>,
) -> Result<Option<GltfAddressabilityBevyAdapterV2>, GltfAddressabilityV2Error> {
    validate_inventory_binding(raw, animations)?;
    let settings = BevyGltfAddressabilitySettingsV2::from_resolved(resolved)?;
    validate_provenance(source, raw, &prediction_provenance)?;
    let rules = BevyGltfAddressabilityRulesV1::frozen(pointer_width);
    let projection = BevyGltfAddressabilityProjectionV2::from_inventories(
        raw,
        animations,
        settings.bevy_animation_feature,
        settings.load_animations,
        pointer_width,
    )?;
    let output = build_check_output(source, raw, animations, &prediction_provenance, &projection)?;
    let check = RichAddressabilityCheck { output };
    let checks: [Box<dyn Check>; 1] = [Box::new(check)];
    let evaluation = evaluate_checks(ctx, &checks, CheckSelection::All)
        .map_err(|_| GltfAddressabilityV2Error::InvalidCheckEvaluation)?
        .pop()
        .expect("one static check produces one evaluation");
    let adapter = GltfAddressabilityBevyAdapterV2 {
        rules,
        settings,
        prediction_provenance,
        check: evaluation,
        projection,
    };
    validate_adapter(
        &adapter,
        &GltfAddressabilityInventoryV2::new(raw.clone(), animations.clone())?,
    )?;
    Ok(Some(adapter))
}

struct RichAddressabilityCheck {
    output: CheckOutput,
}

impl Check for RichAddressabilityCheck {
    fn id(&self) -> &'static str {
        crate::ENGINE_ADDRESSABILITY_CHECK_ID
    }

    fn applicability(&self, _ctx: &CheckCtx<'_>) -> Applicability {
        Applicability::Applicable
    }

    fn evaluate(&self, _ctx: &CheckCtx<'_>) -> CheckOutput {
        self.output.clone()
    }
}

fn build_check_output(
    source: &LoadedSource,
    raw: &RawGltfAddressabilityInventoryV1,
    animations: &GltfAnimationAddressabilityInventoryV1,
    provenance: &PredictionProvenanceV4,
    projection: &BevyGltfAddressabilityProjectionV2,
) -> Result<CheckOutput, GltfAddressabilityV2Error> {
    let facts = source.source_facts();
    let mut scopes = Vec::new();
    let mut facets = Vec::new();
    let mut add = |scope: EvaluationScope, available: bool, reasons: Vec<_>, clip_index| {
        if available {
            scopes.push(scope.clone());
            return Ok::<(), GltfAddressabilityV2Error>(());
        }
        let basis = rich_addressability_basis_v4(facts, clip_index);
        let facet = EnginePredictionFacetV4::required_unavailable(scope, basis, reasons)
            .map_err(|_| GltfAddressabilityV2Error::InvalidCheckEvaluation)?;
        facets.push(facet);
        Ok::<(), GltfAddressabilityV2Error>(())
    };

    if matches!(
        animations.animations().coverage().state(),
        crate::GltfAnimationCoverageStateV1::Complete
    ) {
        for animation in animations.animations().rows() {
            let label = BevyAnimationAssetLabelV1::new(animation.source_clip_index() as usize)
                .map_err(|_| GltfAddressabilityV2Error::InvalidCheckEvaluation)?;
            add(
                EvaluationScope::new(EvaluationScopeCode::ANIMATION_ASSET_LABEL)
                    .subject(label.as_str().to_owned()),
                true,
                Vec::new(),
                Some(animation.source_clip_index() as usize),
            )?;
        }
    } else {
        add(
            EvaluationScope::new(EvaluationScopeCode::ANIMATION_ASSET_LABEL_INVENTORY),
            false,
            vec![PredictionUnavailableReasonV2::RawSourceIncomplete],
            None,
        )?;
    }

    for scene in &projection.scenes {
        add(
            EvaluationScope::new(EvaluationScopeCode::SCENE_ASSET_LABEL)
                .subject(scene.label.clone()),
            true,
            Vec::new(),
            None,
        )?;
    }
    add_projection_facet(
        &mut add,
        EvaluationScope::new(EvaluationScopeCode::DEFAULT_SCENE_ROUTE).subject("default_scene"),
        &projection.default_scene_route,
    )?;
    for skin in &projection.skins {
        add_projection_facet(
            &mut add,
            EvaluationScope::new(EvaluationScopeCode::SKIN_ASSET_LABEL)
                .subject(format!("Skin{}", skin.source_skin_index)),
            &skin.skin_label,
        )?;
        add(
            EvaluationScope::new(EvaluationScopeCode::INVERSE_BIND_MATRICES_ASSET_LABEL)
                .subject(skin.inverse_bind_matrices_label.clone()),
            true,
            Vec::new(),
            None,
        )?;
    }
    for map in &projection.named_maps {
        add_projection_facet(
            &mut add,
            EvaluationScope::new(EvaluationScopeCode::NAMED_ADDRESSABILITY_MAP).subject(match map
                .kind
            {
                GltfAddressabilityNamedMapKindV2::Scene => "scene",
                GltfAddressabilityNamedMapKindV2::Animation => "animation",
                GltfAddressabilityNamedMapKindV2::Skin => "skin",
            }),
            &map.winners,
        )?;
    }
    for target in &projection.targets {
        add_projection_facet(
            &mut add,
            EvaluationScope::new(EvaluationScopeCode::ANIMATION_TARGET_ID)
                .subject(format!("Node{}", target.source_node_index)),
            &target.projection,
        )?;
    }
    if !raw.scene_coverage().is_complete()
        || !raw.node_coverage().is_complete()
        || !raw.skin_coverage().is_complete()
        || !raw.attachment_coverage().is_complete()
        || !raw.path_candidate_coverage().is_complete()
        || animation_inventory_incomplete(animations)
        || projection.target_coverage.is_unavailable()
    {
        add(
            EvaluationScope::new(EvaluationScopeCode::GLTF_ADDRESSABILITY_INVENTORY),
            false,
            vec![PredictionUnavailableReasonV2::RawSourceIncomplete],
            None,
        )?;
    }
    if facets.len() > PREDICTION_V1_MAX_FACETS_PER_FILE {
        // V4 reserves `FacetBudgetExceeded` for this exact check-owned scope;
        // using the otherwise natural glTF-inventory scope would be rejected
        // by the shared CheckEvaluation lifecycle validator.
        let summary_scope = EvaluationScope::new(EvaluationScopeCode::custom(FACET_BUDGET_SCOPE));
        facets = vec![
            EnginePredictionFacetV4::required_unavailable(
                summary_scope,
                rich_addressability_basis_v4(facts, None),
                vec![PredictionUnavailableReasonV2::FacetBudgetExceeded],
            )
            .map_err(|_| GltfAddressabilityV2Error::InvalidCheckEvaluation)?,
        ];
    }
    let output = CheckOutput::from_coverage(Vec::new(), scopes, Vec::new());
    if facets.is_empty() {
        Ok(output)
    } else {
        let prediction = EnginePredictionV4::new(provenance.identity().clone(), facets)
            .map_err(|_| GltfAddressabilityV2Error::InvalidCheckEvaluation)?;
        Ok(output.with_engine_prediction_v4(prediction))
    }
}

fn rich_addressability_basis_v4(
    facts: animsmith_core::SourceFactsViewV1<'_>,
    source_animation_index: Option<usize>,
) -> EnginePredictionBasisV4 {
    let mut references = vec![PredictionBasisReferenceV4::v2(
        PredictionBasisReferenceV2::v1(
            PredictionBasisReferenceV1::primary_source("bevy-gltf-loader-0.19.0-c6f634ca")
                .expect("static primary-source id is valid"),
        ),
    )];
    for setting_id in [
        EngineSettingIdV2::BevyAnimationFeature,
        EngineSettingIdV2::LoadAnimations,
    ] {
        references.push(PredictionBasisReferenceV4::v2(
            PredictionBasisReferenceV2::v1(
                PredictionBasisReferenceV1::resolved_setting(
                    ResolvedSettingLocationV1::Document,
                    setting_id.as_str(),
                )
                .expect("static setting id is valid"),
            ),
        ));
    }
    if let Some(source_animation_index) = source_animation_index {
        let source_name = RawSourceBasisReferenceV1::from_source(
            RawSourceDomainV1::Clip,
            RawSourceKeyV1::Clip {
                source_clip_index: source_animation_index as u64,
            },
            RawSourceFieldIdV1::new("source_name.state").expect("static field is valid"),
            facts,
        )
        .expect("retained animation row resolves its same-load source witness");
        references.push(PredictionBasisReferenceV4::v2(
            PredictionBasisReferenceV2::v1(PredictionBasisReferenceV1::raw_source(source_name)),
        ));
    }
    EnginePredictionBasisV4::new(references).expect("static rich addressability basis is canonical")
}

fn add_projection_facet<T>(
    add: &mut impl FnMut(
        EvaluationScope,
        bool,
        Vec<PredictionUnavailableReasonV2>,
        Option<usize>,
    ) -> Result<(), GltfAddressabilityV2Error>,
    scope: EvaluationScope,
    projection: &GltfAddressabilityProjectionV2<T>,
) -> Result<(), GltfAddressabilityV2Error> {
    match projection {
        GltfAddressabilityProjectionV2::Available { .. }
        | GltfAddressabilityProjectionV2::ProvenAbsent => add(scope, true, Vec::new(), None),
        GltfAddressabilityProjectionV2::RequiredUnavailable { reasons } => add(
            scope,
            false,
            reasons.iter().map(prediction_reason).collect(),
            None,
        ),
    }
}

fn prediction_reason(
    reason: &GltfAddressabilityUnavailableReasonV2,
) -> PredictionUnavailableReasonV2 {
    match reason {
        GltfAddressabilityUnavailableReasonV2::RawSourceIncomplete
        | GltfAddressabilityUnavailableReasonV2::NamedMapIncomplete
        | GltfAddressabilityUnavailableReasonV2::TargetDomainTruncated => {
            PredictionUnavailableReasonV2::RawSourceIncomplete
        }
        GltfAddressabilityUnavailableReasonV2::DependencyClosureIncomplete => {
            PredictionUnavailableReasonV2::DependencyClosureIncomplete
        }
        GltfAddressabilityUnavailableReasonV2::BevyAnimationFeatureDisabled
        | GltfAddressabilityUnavailableReasonV2::LoadAnimationsDisabled
        | GltfAddressabilityUnavailableReasonV2::TargetPointerWidthMissing => {
            PredictionUnavailableReasonV2::ProjectIntentUnavailable
        }
        GltfAddressabilityUnavailableReasonV2::UnreachableTarget => {
            PredictionUnavailableReasonV2::SourceSelectorNoMatch
        }
        GltfAddressabilityUnavailableReasonV2::MultipleCandidatePaths
        | GltfAddressabilityUnavailableReasonV2::DuplicateFullPath
        | GltfAddressabilityUnavailableReasonV2::TargetIdCollision => {
            PredictionUnavailableReasonV2::SourceSelectorAmbiguous
        }
        GltfAddressabilityUnavailableReasonV2::PathBoundsExceeded
        | GltfAddressabilityUnavailableReasonV2::ProjectionBoundsExceeded => {
            PredictionUnavailableReasonV2::MeasurementUnavailable
        }
    }
}

fn validate_provenance(
    source: &LoadedSource,
    raw: &RawGltfAddressabilityInventoryV1,
    provenance: &PredictionProvenanceV4,
) -> Result<(), GltfAddressabilityV2Error> {
    provenance
        .validate()
        .map_err(|_| GltfAddressabilityV2Error::InvalidPredictionProvenance)?;
    let selection = provenance.profile().selection();
    if selection.family() != "bevy"
        || selection.profile_revision() != BEVY_PROFILE_REVISION
        || selection.engine_version() != "0.19.0"
        || selection.importer() != "gltf-asset-loader"
    {
        return Err(GltfAddressabilityV2Error::InvalidBevyProfile);
    }
    if provenance.raw_source().primary_input() != raw.primary_input()
        || provenance.dependency_closure() != raw.dependency_closure()
        || source.dependency_closure() != raw.dependency_closure()
        || source.source_facts().primary_identity() != raw.primary_input()
    {
        return Err(GltfAddressabilityV2Error::InventoryBindingMismatch);
    }
    Ok(())
}

fn validate_adapter(
    adapter: &GltfAddressabilityBevyAdapterV2,
    inventory: &GltfAddressabilityInventoryV2,
) -> Result<(), GltfAddressabilityV2Error> {
    inventory.validate()?;
    if !adapter.rules.validate() {
        return Err(GltfAddressabilityV2Error::InvalidRuleBundle);
    }
    adapter
        .prediction_provenance
        .validate()
        .map_err(|_| GltfAddressabilityV2Error::InvalidPredictionProvenance)?;
    let selection = adapter.prediction_provenance.profile().selection();
    if selection.family() != "bevy"
        || selection.profile_revision() != BEVY_PROFILE_REVISION
        || selection.engine_version() != "0.19.0"
        || selection.importer() != "gltf-asset-loader"
        || adapter.prediction_provenance.raw_source().primary_input()
            != inventory.raw.primary_input()
        || adapter.prediction_provenance.dependency_closure() != inventory.raw.dependency_closure()
    {
        return Err(GltfAddressabilityV2Error::InvalidPredictionProvenance);
    }
    validate_settings_binding(&adapter.settings, &adapter.prediction_provenance)?;
    let expected = BevyGltfAddressabilityProjectionV2::from_inventories(
        &inventory.raw,
        &inventory.animations,
        adapter.settings.bevy_animation_feature,
        adapter.settings.load_animations,
        adapter.rules.target_pointer_width,
    )?;
    if expected != adapter.projection
        || adapter.check.check_id() != crate::ENGINE_ADDRESSABILITY_CHECK_ID
        || adapter.check.selection() != SelectionState::Selected
        || adapter.check.configuration() != ConfigurationState::Enabled
        || adapter.check.applicability() != Applicability::Applicable
        || !adapter.check.findings().is_empty()
        || !adapter.check.gaps().is_empty()
    {
        return Err(GltfAddressabilityV2Error::InvalidCheckEvaluation);
    }
    let prediction = adapter.check.engine_prediction_v4();
    if let Some(prediction) = prediction {
        prediction
            .validate_against_provenance(&adapter.prediction_provenance)
            .map_err(|_| GltfAddressabilityV2Error::InvalidCheckEvaluation)?;
    }
    validate_facet_states(
        prediction,
        adapter.check.evaluated_scopes(),
        adapter.check.evaluation(),
        &expected_facet_specs(inventory, &adapter.projection),
    )
}

fn validate_settings_binding(
    settings: &BevyGltfAddressabilitySettingsV2,
    provenance: &PredictionProvenanceV4,
) -> Result<(), GltfAddressabilityV2Error> {
    let matches = |id, expected_value, expected_origin| {
        provenance
            .settings()
            .document_setting(id)
            .is_some_and(|row| {
                row.value() == &EngineSettingValueV2::Boolean(expected_value)
                    && row.value_origin()
                        == match expected_origin {
                            GltfAddressabilitySettingOriginV2::ExplicitConfig => {
                                EngineSettingValueOriginV3::ExplicitConfig
                            }
                            GltfAddressabilitySettingOriginV2::ProfileDefault => {
                                EngineSettingValueOriginV3::ProfileDefault
                            }
                        }
            })
    };
    if !matches(
        EngineSettingIdV2::BevyAnimationFeature,
        settings.bevy_animation_feature,
        settings.bevy_animation_feature_origin,
    ) || !matches(
        EngineSettingIdV2::LoadAnimations,
        settings.load_animations,
        settings.load_animations_origin,
    ) {
        return Err(GltfAddressabilityV2Error::InvalidPredictionProvenance);
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct ExpectedFacetV2 {
    scope: EvaluationScope,
    state: EnginePredictionFacetStateV1,
    reasons: Vec<PredictionUnavailableReasonV2>,
}

fn validate_facet_states(
    prediction: Option<&EnginePredictionV4>,
    evaluated_scopes: &[EvaluationScope],
    evaluation: EvaluationState,
    expected: &[ExpectedFacetV2],
) -> Result<(), GltfAddressabilityV2Error> {
    if evaluated_scopes.len() > GLTF_ADDRESSABILITY_V2_MAX_EVALUATED_SCOPES {
        return Err(GltfAddressabilityV2Error::InvalidCheckEvaluation);
    }
    let expected_available = expected
        .iter()
        .filter(|facet| facet.state == EnginePredictionFacetStateV1::Available)
        .collect::<Vec<_>>();
    let expected_unavailable = expected
        .iter()
        .filter(|facet| facet.state == EnginePredictionFacetStateV1::RequiredPredictionUnavailable)
        .collect::<Vec<_>>();
    let actual_facets = prediction.map_or(&[][..], EnginePredictionV4::facets);
    let compacted = expected_unavailable.len() > PREDICTION_V1_MAX_FACETS_PER_FILE;

    if compacted {
        if actual_facets.len() != 1
            || actual_facets[0].scope().code.as_str() != FACET_BUDGET_SCOPE
            || actual_facets[0].scope().subject.is_some()
            || actual_facets[0].state()
                != EnginePredictionFacetStateV1::RequiredPredictionUnavailable
            || actual_facets[0].result().is_some()
            || actual_facets[0].reasons() != [PredictionUnavailableReasonV2::FacetBudgetExceeded]
            || evaluated_scopes.len() != expected_available.len()
            || evaluated_scopes.iter().enumerate().any(|(index, scope)| {
                evaluated_scopes[..index].contains(scope)
                    || !expected_available
                        .iter()
                        .any(|expected| expected.scope == *scope)
            })
        {
            return Err(GltfAddressabilityV2Error::InvalidCheckEvaluation);
        }
        let expected_evaluation = if expected_available.is_empty() {
            EvaluationState::NotEvaluated
        } else {
            EvaluationState::Partial
        };
        return if evaluation == expected_evaluation {
            Ok(())
        } else {
            Err(GltfAddressabilityV2Error::InvalidCheckEvaluation)
        };
    }

    if evaluated_scopes.len() != expected_available.len()
        || actual_facets.len() != expected_unavailable.len()
        || evaluated_scopes
            .iter()
            .enumerate()
            .any(|(index, scope)| evaluated_scopes[..index].contains(scope))
        || evaluated_scopes
            .iter()
            .any(|scope| actual_facets.iter().any(|facet| facet.scope() == scope))
    {
        return Err(GltfAddressabilityV2Error::InvalidCheckEvaluation);
    }

    for expected in &expected_available {
        if evaluated_scopes
            .iter()
            .filter(|scope| *scope == &expected.scope)
            .count()
            != 1
        {
            return Err(GltfAddressabilityV2Error::InvalidCheckEvaluation);
        }
    }
    for expected in &expected_unavailable {
        let mut matching = actual_facets
            .iter()
            .filter(|facet| facet.scope() == &expected.scope);
        let Some(actual) = matching.next() else {
            return Err(GltfAddressabilityV2Error::InvalidCheckEvaluation);
        };
        if matching.next().is_some()
            || actual.state() != EnginePredictionFacetStateV1::RequiredPredictionUnavailable
            || actual.result().is_some()
            || actual.reasons() != expected.reasons
        {
            return Err(GltfAddressabilityV2Error::InvalidCheckEvaluation);
        }
    }

    if actual_facets.iter().any(|facet| {
        facet.state() != EnginePredictionFacetStateV1::RequiredPredictionUnavailable
            || !expected
                .iter()
                .any(|expected| expected.scope == *facet.scope())
    }) || evaluated_scopes
        .iter()
        .any(|scope| !expected.iter().any(|expected| expected.scope == *scope))
    {
        return Err(GltfAddressabilityV2Error::InvalidCheckEvaluation);
    }

    let expected_evaluation = if expected_unavailable.is_empty() {
        EvaluationState::Complete
    } else if expected_available.is_empty() {
        EvaluationState::NotEvaluated
    } else {
        EvaluationState::Partial
    };
    if evaluation != expected_evaluation {
        return Err(GltfAddressabilityV2Error::InvalidCheckEvaluation);
    }
    Ok(())
}

fn expected_facet_specs(
    inventory: &GltfAddressabilityInventoryV2,
    projection: &BevyGltfAddressabilityProjectionV2,
) -> Vec<ExpectedFacetV2> {
    let mut expected = Vec::new();
    if matches!(
        inventory.animations.animations().coverage().state(),
        crate::GltfAnimationCoverageStateV1::Complete
    ) {
        for animation in inventory.animations.animations().rows() {
            expected.push(ExpectedFacetV2 {
                scope: EvaluationScope::new(EvaluationScopeCode::ANIMATION_ASSET_LABEL)
                    .subject(format!("Animation{}", animation.source_clip_index())),
                state: EnginePredictionFacetStateV1::Available,
                reasons: Vec::new(),
            });
        }
    } else {
        expected.push(ExpectedFacetV2 {
            scope: EvaluationScope::new(EvaluationScopeCode::ANIMATION_ASSET_LABEL_INVENTORY),
            state: EnginePredictionFacetStateV1::RequiredPredictionUnavailable,
            reasons: vec![PredictionUnavailableReasonV2::RawSourceIncomplete],
        });
    }
    for scene in &projection.scenes {
        expected.push(ExpectedFacetV2 {
            scope: EvaluationScope::new(EvaluationScopeCode::SCENE_ASSET_LABEL)
                .subject(scene.label.clone()),
            state: EnginePredictionFacetStateV1::Available,
            reasons: Vec::new(),
        });
    }
    push_expected_projection(
        &mut expected,
        EvaluationScope::new(EvaluationScopeCode::DEFAULT_SCENE_ROUTE).subject("default_scene"),
        &projection.default_scene_route,
    );
    for skin in &projection.skins {
        push_expected_projection(
            &mut expected,
            EvaluationScope::new(EvaluationScopeCode::SKIN_ASSET_LABEL)
                .subject(format!("Skin{}", skin.source_skin_index)),
            &skin.skin_label,
        );
        expected.push(ExpectedFacetV2 {
            scope: EvaluationScope::new(EvaluationScopeCode::INVERSE_BIND_MATRICES_ASSET_LABEL)
                .subject(skin.inverse_bind_matrices_label.clone()),
            state: EnginePredictionFacetStateV1::Available,
            reasons: Vec::new(),
        });
    }
    for map in &projection.named_maps {
        let subject = match map.kind {
            GltfAddressabilityNamedMapKindV2::Scene => "scene",
            GltfAddressabilityNamedMapKindV2::Animation => "animation",
            GltfAddressabilityNamedMapKindV2::Skin => "skin",
        };
        push_expected_projection(
            &mut expected,
            EvaluationScope::new(EvaluationScopeCode::NAMED_ADDRESSABILITY_MAP).subject(subject),
            &map.winners,
        );
    }
    for target in &projection.targets {
        push_expected_projection(
            &mut expected,
            EvaluationScope::new(EvaluationScopeCode::ANIMATION_TARGET_ID)
                .subject(format!("Node{}", target.source_node_index)),
            &target.projection,
        );
    }
    if !inventory.raw.scene_coverage().is_complete()
        || !inventory.raw.node_coverage().is_complete()
        || !inventory.raw.skin_coverage().is_complete()
        || !inventory.raw.attachment_coverage().is_complete()
        || !inventory.raw.path_candidate_coverage().is_complete()
        || animation_inventory_incomplete(&inventory.animations)
        || projection.target_coverage.is_unavailable()
    {
        expected.push(ExpectedFacetV2 {
            scope: EvaluationScope::new(EvaluationScopeCode::GLTF_ADDRESSABILITY_INVENTORY),
            state: EnginePredictionFacetStateV1::RequiredPredictionUnavailable,
            reasons: vec![PredictionUnavailableReasonV2::RawSourceIncomplete],
        });
    }
    expected
}

fn animation_inventory_incomplete(animations: &GltfAnimationAddressabilityInventoryV1) -> bool {
    !matches!(
        animations.animations().coverage().state(),
        crate::GltfAnimationCoverageStateV1::Complete
    ) || animations.animations().rows().iter().any(|animation| {
        !matches!(
            animation.channels().coverage().state(),
            crate::GltfAnimationCoverageStateV1::Complete
        )
    })
}

fn push_expected_projection<T>(
    expected: &mut Vec<ExpectedFacetV2>,
    scope: EvaluationScope,
    projection: &GltfAddressabilityProjectionV2<T>,
) {
    let (state, mut reasons) = match projection {
        GltfAddressabilityProjectionV2::Available { .. }
        | GltfAddressabilityProjectionV2::ProvenAbsent => {
            (EnginePredictionFacetStateV1::Available, Vec::new())
        }
        GltfAddressabilityProjectionV2::RequiredUnavailable { reasons } => (
            EnginePredictionFacetStateV1::RequiredPredictionUnavailable,
            reasons.iter().map(prediction_reason).collect(),
        ),
    };
    reasons.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    reasons.dedup();
    expected.push(ExpectedFacetV2 {
        scope,
        state,
        reasons,
    });
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolSourceWireV2 {
    revision: Option<String>,
    dirty: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolWireV2 {
    name: String,
    version: String,
    source: ToolSourceWireV2,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdapterWireV2 {
    rules: BevyGltfAddressabilityRulesV1,
    settings: BevyGltfAddressabilitySettingsV2,
    prediction_provenance: PredictionProvenanceV4,
    check: Box<RawValue>,
    projection: BevyGltfAddressabilityProjectionV2,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GltfAddressabilityWireV2 {
    schema_version: u32,
    schema: String,
    tool: ToolWireV2,
    command: String,
    input: InputIdentity,
    inventory: GltfAddressabilityInventoryV2,
    bevy: Option<AdapterWireV2>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckWireV2 {
    check_id: String,
    selection: SelectionState,
    configuration: ConfigurationState,
    applicability: Applicability,
    evaluation: EvaluationState,
    findings: Vec<serde_json::Value>,
    #[serde(default)]
    evaluated_scopes: Vec<EvaluationScope>,
    #[serde(default)]
    gaps: Vec<serde_json::Value>,
    #[serde(default)]
    prediction: Option<EnginePredictionV4>,
}

/// Strict read-side representation of the one rich check evaluation.
#[derive(Debug, Clone)]
pub struct GltfAddressabilityCheckReadbackV2 {
    check_id: String,
    selection: SelectionState,
    configuration: ConfigurationState,
    applicability: Applicability,
    evaluation: EvaluationState,
    evaluated_scopes: Vec<EvaluationScope>,
    prediction: Option<EnginePredictionV4>,
}

impl GltfAddressabilityCheckReadbackV2 {
    /// Stable check id, validated as `engine-addressability`.
    pub fn check_id(&self) -> &str {
        &self.check_id
    }

    /// Serialized selection state.
    pub const fn selection(&self) -> SelectionState {
        self.selection
    }

    /// Serialized configuration state.
    pub const fn configuration(&self) -> ConfigurationState {
        self.configuration
    }

    /// Serialized applicability state.
    pub const fn applicability(&self) -> Applicability {
        self.applicability
    }

    /// Derived serialized evaluation state.
    pub const fn evaluation(&self) -> EvaluationState {
        self.evaluation
    }

    /// Strictly decoded V4 prediction attachment.
    pub const fn prediction(&self) -> Option<&EnginePredictionV4> {
        self.prediction.as_ref()
    }

    /// Exact completed scopes.
    pub fn evaluated_scopes(&self) -> &[EvaluationScope] {
        &self.evaluated_scopes
    }
}

/// Strict read-side exact Bevy adapter.
#[derive(Debug, Clone)]
pub struct GltfAddressabilityBevyReadbackV2 {
    rules: BevyGltfAddressabilityRulesV1,
    settings: BevyGltfAddressabilitySettingsV2,
    prediction_provenance: PredictionProvenanceV4,
    check: GltfAddressabilityCheckReadbackV2,
    projection: BevyGltfAddressabilityProjectionV2,
}

impl GltfAddressabilityBevyReadbackV2 {
    /// Frozen rule bundle.
    pub const fn rules(&self) -> &BevyGltfAddressabilityRulesV1 {
        &self.rules
    }

    /// Exact consumed settings.
    pub const fn settings(&self) -> &BevyGltfAddressabilitySettingsV2 {
        &self.settings
    }

    /// Strict same-load provenance.
    pub const fn prediction_provenance(&self) -> &PredictionProvenanceV4 {
        &self.prediction_provenance
    }

    /// Strict real-check readback.
    pub const fn check(&self) -> &GltfAddressabilityCheckReadbackV2 {
        &self.check
    }

    /// Structured rich projections.
    pub const fn projection(&self) -> &BevyGltfAddressabilityProjectionV2 {
        &self.projection
    }
}

/// Fully validated read-side V2 addressability report.
#[derive(Debug, Clone)]
pub struct GltfAddressabilityReadbackV2 {
    input: InputIdentity,
    inventory: GltfAddressabilityInventoryV2,
    bevy: Option<GltfAddressabilityBevyReadbackV2>,
}

impl GltfAddressabilityReadbackV2 {
    fn from_wire(wire: GltfAddressabilityWireV2) -> Result<Self, GltfAddressabilityV2Error> {
        if wire.schema_version != GLTF_ADDRESSABILITY_V2_SCHEMA_VERSION
            || wire.schema != GLTF_ADDRESSABILITY_V2_ID
            || wire.command != crate::GLTF_ANIMATION_ADDRESSABILITY_COMMAND
            || wire.tool.name != "animsmith"
            || !valid_semver(&wire.tool.version)
            || wire.tool.source.revision.as_ref().is_some_and(|revision| {
                revision.len() != 40 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        {
            return Err(GltfAddressabilityV2Error::InvalidSchema);
        }
        let _dirty = wire.tool.source.dirty;
        wire.inventory.validate()?;
        if &wire.input != wire.inventory.raw.primary_input() {
            return Err(GltfAddressabilityV2Error::InventoryBindingMismatch);
        }
        let bevy = wire
            .bevy
            .map(|adapter| read_adapter(adapter, &wire.inventory))
            .transpose()?;
        Ok(Self {
            input: wire.input,
            inventory: wire.inventory,
            bevy,
        })
    }

    /// Exact primary input identity.
    pub const fn input(&self) -> &InputIdentity {
        &self.input
    }

    /// Strict rich inventory.
    pub const fn inventory(&self) -> &GltfAddressabilityInventoryV2 {
        &self.inventory
    }

    /// Strict exact Bevy adapter when present.
    pub const fn bevy(&self) -> Option<&GltfAddressabilityBevyReadbackV2> {
        self.bevy.as_ref()
    }
}

fn read_adapter(
    wire: AdapterWireV2,
    inventory: &GltfAddressabilityInventoryV2,
) -> Result<GltfAddressabilityBevyReadbackV2, GltfAddressabilityV2Error> {
    if !wire.rules.validate() {
        return Err(GltfAddressabilityV2Error::InvalidRuleBundle);
    }
    wire.prediction_provenance
        .validate()
        .map_err(|_| GltfAddressabilityV2Error::InvalidPredictionProvenance)?;
    let selection = wire.prediction_provenance.profile().selection();
    if selection.family() != "bevy"
        || selection.profile_revision() != BEVY_PROFILE_REVISION
        || selection.engine_version() != "0.19.0"
        || selection.importer() != "gltf-asset-loader"
        || wire.prediction_provenance.raw_source().primary_input() != inventory.raw.primary_input()
        || wire.prediction_provenance.dependency_closure() != inventory.raw.dependency_closure()
    {
        return Err(GltfAddressabilityV2Error::InvalidPredictionProvenance);
    }
    validate_settings_binding(&wire.settings, &wire.prediction_provenance)?;
    let expected = BevyGltfAddressabilityProjectionV2::from_inventories(
        &inventory.raw,
        &inventory.animations,
        wire.settings.bevy_animation_feature,
        wire.settings.load_animations,
        wire.rules.target_pointer_width,
    )?;
    if expected != wire.projection {
        return Err(GltfAddressabilityV2Error::InvalidCheckEvaluation);
    }
    let check: CheckWireV2 = serde_json::from_str(wire.check.get())
        .map_err(|_| GltfAddressabilityV2Error::InvalidCheckEvaluation)?;
    if check.check_id != crate::ENGINE_ADDRESSABILITY_CHECK_ID
        || check.selection != SelectionState::Selected
        || check.configuration != ConfigurationState::Enabled
        || check.applicability != Applicability::Applicable
        || !check.findings.is_empty()
        || !check.gaps.is_empty()
    {
        return Err(GltfAddressabilityV2Error::InvalidCheckEvaluation);
    }
    if let Some(prediction) = &check.prediction {
        prediction
            .validate_against_provenance(&wire.prediction_provenance)
            .map_err(|_| GltfAddressabilityV2Error::InvalidCheckEvaluation)?;
    }
    validate_facet_states(
        check.prediction.as_ref(),
        &check.evaluated_scopes,
        check.evaluation,
        &expected_facet_specs(inventory, &wire.projection),
    )?;
    Ok(GltfAddressabilityBevyReadbackV2 {
        rules: wire.rules,
        settings: wire.settings,
        prediction_provenance: wire.prediction_provenance,
        check: GltfAddressabilityCheckReadbackV2 {
            check_id: check.check_id,
            selection: check.selection,
            configuration: check.configuration,
            applicability: check.applicability,
            evaluation: check.evaluation,
            evaluated_scopes: check.evaluated_scopes,
            prediction: check.prediction,
        },
        projection: wire.projection,
    })
}

fn valid_semver(value: &str) -> bool {
    let core = value
        .split_once('+')
        .map_or(value, |(core, _)| core)
        .split_once('-')
        .map_or_else(
            || value.split_once('+').map_or(value, |(core, _)| core),
            |(core, _)| core,
        );
    let mut parts = core.split('.');
    (0..3).all(|_| {
        parts
            .next()
            .is_some_and(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    }) && parts.next().is_none()
}

/// Bounded rich report reader failure.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum GltfAddressabilityReadErrorV2 {
    /// Reading the bounded input failed.
    #[error("cannot read rich addressability report: {source}")]
    Io {
        /// Underlying reader error.
        source: std::io::Error,
    },
    /// The serialized input exceeded 256 MiB.
    #[error("rich addressability report exceeds byte limit {limit}")]
    ReportTooLarge {
        /// Immutable byte ceiling.
        limit: u64,
    },
    /// Strict JSON decoding failed.
    #[error("invalid rich addressability JSON: {source}")]
    InvalidJson {
        /// Decoder diagnostic.
        source: serde_json::Error,
    },
    /// Cross-field or identity validation failed.
    #[error(transparent)]
    Contract(#[from] GltfAddressabilityV2Error),
}

fn validate_inventory_binding(
    raw: &RawGltfAddressabilityInventoryV1,
    animations: &GltfAnimationAddressabilityInventoryV1,
) -> Result<(), GltfAddressabilityV2Error> {
    raw.validate()
        .map_err(|_| GltfAddressabilityV2Error::InvalidRawInventory)?;
    if raw.primary_input() != animations.primary_input()
        || raw.dependency_closure() != animations.dependency_closure()
    {
        return Err(GltfAddressabilityV2Error::InventoryBindingMismatch);
    }
    Ok(())
}

fn project_named_maps(
    raw: &RawGltfAddressabilityInventoryV1,
    animations: &GltfAnimationAddressabilityInventoryV1,
) -> Vec<GltfAddressabilityNamedMapV2> {
    let mut scene_winners = BTreeMap::new();
    for row in raw.scenes() {
        if let Some(name) = row.name() {
            scene_winners.insert(
                name.to_owned(),
                GltfAddressabilityNamedMapWinnerV2 {
                    name: name.to_owned(),
                    source_index: row.source_scene_index(),
                    typed_label: format!("Scene{}", row.source_scene_index()),
                },
            );
        }
    }
    let scenes = named_map(
        GltfAddressabilityNamedMapKindV2::Scene,
        raw.scene_coverage().is_complete(),
        scene_winners,
    );

    let mut animation_winners = BTreeMap::new();
    for row in animations.animations().rows() {
        if let GltfAnimationObservationV1::Observed { value: name } = row.source_name() {
            animation_winners.insert(
                name.clone(),
                GltfAddressabilityNamedMapWinnerV2 {
                    name: name.clone(),
                    source_index: row.source_clip_index(),
                    typed_label: format!("Animation{}", row.source_clip_index()),
                },
            );
        }
    }
    let animations = named_map(
        GltfAddressabilityNamedMapKindV2::Animation,
        matches!(
            animations.animations().coverage().state(),
            crate::GltfAnimationCoverageStateV1::Complete
        ),
        animation_winners,
    );

    // Skin handles are lazily created in first-reference order, and the named
    // map is last-write-wins over that order.  Unreferenced skins never enter
    // this map even though their inverse-bind assets are created eagerly.
    let skins_by_index = raw
        .skins()
        .iter()
        .map(|skin| (skin.source_skin_index(), skin))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    let mut skin_winners = BTreeMap::new();
    for attachment in raw.attachments() {
        let index = attachment.source_skin_index();
        if seen.insert(index)
            && let Some(skin) = skins_by_index.get(&index)
            && let Some(name) = skin.name()
        {
            skin_winners.insert(
                name.to_owned(),
                GltfAddressabilityNamedMapWinnerV2 {
                    name: name.to_owned(),
                    source_index: index,
                    typed_label: format!("Skin{index}"),
                },
            );
        }
    }
    let skins = named_map(
        GltfAddressabilityNamedMapKindV2::Skin,
        raw.skin_coverage().is_complete() && raw.attachment_coverage().is_complete(),
        skin_winners,
    );
    vec![scenes, animations, skins]
}

fn named_map(
    kind: GltfAddressabilityNamedMapKindV2,
    complete: bool,
    winners: BTreeMap<String, GltfAddressabilityNamedMapWinnerV2>,
) -> GltfAddressabilityNamedMapV2 {
    let keys_bounded = winners
        .keys()
        .all(|name| name.len() <= GLTF_ADDRESSABILITY_V2_MAX_SEGMENT_BYTES);
    GltfAddressabilityNamedMapV2 {
        kind,
        duplicate_policy: BevyNamedMapDuplicatePolicyV1::LastWriteWins,
        winners: if !keys_bounded {
            GltfAddressabilityProjectionV2::unavailable(vec![
                GltfAddressabilityUnavailableReasonV2::ProjectionBoundsExceeded,
            ])
        } else if complete {
            GltfAddressabilityProjectionV2::Available {
                value: winners.into_values().collect(),
            }
        } else {
            GltfAddressabilityProjectionV2::unavailable(vec![
                GltfAddressabilityUnavailableReasonV2::NamedMapIncomplete,
            ])
        },
    }
}

fn normalize_projection_bounds(projection: &mut BevyGltfAddressabilityProjectionV2) {
    if projection_structural_references(projection)
        > GLTF_ADDRESSABILITY_V2_MAX_STRUCTURAL_REFERENCES
    {
        make_named_maps_bounds_unavailable(projection);
        let mut retained_references = 0usize;
        let mut retained = 0usize;
        for target in &projection.targets {
            let target_references = target.contributing_channels.len()
                + match &target.projection {
                    GltfAddressabilityProjectionV2::Available { value } => value.segments.len(),
                    GltfAddressabilityProjectionV2::ProvenAbsent
                    | GltfAddressabilityProjectionV2::RequiredUnavailable { .. } => 0,
                };
            if retained_references.saturating_add(target_references)
                > GLTF_ADDRESSABILITY_V2_MAX_STRUCTURAL_REFERENCES
            {
                break;
            }
            retained_references += target_references;
            retained += 1;
        }
        if retained < projection.targets.len() {
            projection.targets.truncate(retained);
            projection.target_coverage = GltfAddressabilityProjectionV2::unavailable(vec![
                GltfAddressabilityUnavailableReasonV2::ProjectionBoundsExceeded,
            ]);
        }
        // Any omitted structure can hide a path or UUID collision.
        if projection.target_coverage.is_unavailable() {
            make_targets_bounds_unavailable(projection);
        }
    }
    if projection_text_bytes(projection) > GLTF_ADDRESSABILITY_V2_MAX_TOTAL_TEXT_BYTES {
        make_named_maps_bounds_unavailable(projection);
        make_targets_bounds_unavailable(projection);
    }
    debug_assert!(
        projection_structural_references(projection)
            <= GLTF_ADDRESSABILITY_V2_MAX_STRUCTURAL_REFERENCES
    );
    debug_assert!(projection_text_bytes(projection) <= GLTF_ADDRESSABILITY_V2_MAX_TOTAL_TEXT_BYTES);
}

fn make_named_maps_bounds_unavailable(projection: &mut BevyGltfAddressabilityProjectionV2) {
    for map in &mut projection.named_maps {
        map.winners = GltfAddressabilityProjectionV2::unavailable(vec![
            GltfAddressabilityUnavailableReasonV2::ProjectionBoundsExceeded,
        ]);
    }
}

fn make_targets_bounds_unavailable(projection: &mut BevyGltfAddressabilityProjectionV2) {
    projection.target_coverage = GltfAddressabilityProjectionV2::unavailable(vec![
        GltfAddressabilityUnavailableReasonV2::ProjectionBoundsExceeded,
    ]);
    for target in &mut projection.targets {
        target.projection = GltfAddressabilityProjectionV2::unavailable(vec![
            GltfAddressabilityUnavailableReasonV2::ProjectionBoundsExceeded,
        ]);
    }
}

fn projection_structural_references(projection: &BevyGltfAddressabilityProjectionV2) -> usize {
    let named = projection
        .named_maps
        .iter()
        .map(|map| match &map.winners {
            GltfAddressabilityProjectionV2::Available { value } => value.len(),
            GltfAddressabilityProjectionV2::ProvenAbsent
            | GltfAddressabilityProjectionV2::RequiredUnavailable { .. } => 0,
        })
        .sum::<usize>();
    named
        + projection
            .targets
            .iter()
            .map(|target| {
                target.contributing_channels.len()
                    + match &target.projection {
                        GltfAddressabilityProjectionV2::Available { value } => value.segments.len(),
                        GltfAddressabilityProjectionV2::ProvenAbsent
                        | GltfAddressabilityProjectionV2::RequiredUnavailable { .. } => 0,
                    }
            })
            .sum::<usize>()
}

fn projection_text_bytes(projection: &BevyGltfAddressabilityProjectionV2) -> usize {
    let mut bytes = projection
        .scenes
        .iter()
        .map(|row| row.label.len())
        .sum::<usize>();
    if let GltfAddressabilityProjectionV2::Available { value } = &projection.default_scene_route {
        bytes = bytes.saturating_add(value.len());
    }
    for skin in &projection.skins {
        bytes = bytes.saturating_add(skin.inverse_bind_matrices_label.len());
        if let GltfAddressabilityProjectionV2::Available { value } = &skin.skin_label {
            bytes = bytes.saturating_add(value.len());
        }
    }
    for map in &projection.named_maps {
        if let GltfAddressabilityProjectionV2::Available { value } = &map.winners {
            for winner in value {
                bytes = bytes
                    .saturating_add(winner.name.len())
                    .saturating_add(winner.typed_label.len());
            }
        }
    }
    for target in &projection.targets {
        if let GltfAddressabilityProjectionV2::Available { value } = &target.projection {
            bytes = bytes
                .saturating_add(value.path.len())
                .saturating_add(value.uuid.len())
                .saturating_add(value.segments.iter().map(String::len).sum::<usize>());
        }
    }
    bytes
}

fn project_targets(
    raw: &RawGltfAddressabilityInventoryV1,
    animations: &GltfAnimationAddressabilityInventoryV1,
    bevy_animation_enabled: bool,
    load_animations: bool,
    pointer_width: Option<TargetPointerWidth>,
) -> Result<
    (
        GltfAddressabilityProjectionV2<()>,
        Vec<GltfAddressabilityTargetV2>,
    ),
    GltfAddressabilityV2Error,
> {
    let mut contributors = BTreeMap::<u64, Vec<GltfAddressabilityTargetChannelV2>>::new();
    for animation in animations.animations().rows() {
        for channel in animation.channels().rows() {
            contributors
                .entry(channel.target().index())
                .or_default()
                .push(GltfAddressabilityTargetChannelV2 {
                    source_animation_index: animation.source_clip_index(),
                    source_channel_index: channel.source_channel_index(),
                });
        }
    }
    let truncated = contributors.len() > GLTF_ADDRESSABILITY_V2_MAX_DOMAIN_ROWS;
    let mut candidate_paths = BTreeMap::<u64, BTreeSet<Vec<u64>>>::new();
    for candidate in raw.path_candidates() {
        if let Some(target) = candidate.target_node_index() {
            candidate_paths
                .entry(target)
                .or_default()
                .insert(candidate.source_node_indices().to_vec());
        }
    }
    let nodes = raw
        .nodes()
        .iter()
        .map(|node| (node.source_node_index(), node))
        .collect::<BTreeMap<_, _>>();
    let base_reasons = target_base_reasons(
        raw,
        animations,
        bevy_animation_enabled,
        load_animations,
        pointer_width,
    );
    let mut targets = Vec::with_capacity(
        contributors
            .len()
            .min(GLTF_ADDRESSABILITY_V2_MAX_DOMAIN_ROWS),
    );
    for (source_node_index, contributing_channels) in contributors
        .into_iter()
        .take(GLTF_ADDRESSABILITY_V2_MAX_DOMAIN_ROWS)
    {
        let mut reasons = base_reasons.clone();
        let paths = candidate_paths.get(&source_node_index);
        let unique_path = match paths {
            None => {
                reasons.push(GltfAddressabilityUnavailableReasonV2::UnreachableTarget);
                None
            }
            Some(paths) if paths.len() != 1 => {
                reasons.push(GltfAddressabilityUnavailableReasonV2::MultipleCandidatePaths);
                None
            }
            Some(paths) => paths.first(),
        };
        let projection = if reasons.is_empty() {
            let node_path = unique_path.expect("one path exists without path reasons");
            let segments = node_path
                .iter()
                .map(|index| {
                    nodes
                        .get(index)
                        .map(|node| {
                            node.name()
                                .map_or_else(|| format!("GltfNode{index}"), str::to_owned)
                        })
                        .ok_or(GltfAddressabilityV2Error::InventoryBindingMismatch)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let path = segments.join("/");
            let width = pointer_width.expect("width is present without base reason");
            match bevy_animation_target_id_v1(segments.iter().map(String::as_str), width) {
                Ok(uuid) => GltfAddressabilityProjectionV2::Available {
                    value: GltfAddressabilityTargetValueV2 {
                        segments,
                        path,
                        uuid,
                    },
                },
                Err(_) => GltfAddressabilityProjectionV2::unavailable(vec![
                    GltfAddressabilityUnavailableReasonV2::PathBoundsExceeded,
                ]),
            }
        } else {
            GltfAddressabilityProjectionV2::unavailable(reasons)
        };
        targets.push(GltfAddressabilityTargetV2 {
            source_node_index,
            contributing_channels,
            projection,
        });
    }
    if truncated {
        // Collision freedom is a whole-domain property. Once the canonical
        // tail is omitted, no retained path/UUID is exact even if it is unique
        // within the retained prefix.
        for target in &mut targets {
            target.projection = GltfAddressabilityProjectionV2::unavailable(vec![
                GltfAddressabilityUnavailableReasonV2::TargetDomainTruncated,
            ]);
        }
    } else if targets.iter().any(|target| {
        matches!(
            &target.projection,
            GltfAddressabilityProjectionV2::RequiredUnavailable { reasons }
                if reasons.contains(&GltfAddressabilityUnavailableReasonV2::PathBoundsExceeded)
        )
    }) {
        // A path outside this contract's hashing bound can still participate
        // in Bevy's global UUID domain, so collision freedom is unavailable
        // for every otherwise exact retained target.
        for target in &mut targets {
            if matches!(
                target.projection,
                GltfAddressabilityProjectionV2::Available { .. }
            ) {
                target.projection = GltfAddressabilityProjectionV2::unavailable(vec![
                    GltfAddressabilityUnavailableReasonV2::ProjectionBoundsExceeded,
                ]);
            }
        }
    } else {
        invalidate_target_collisions(&mut targets);
    }
    let coverage = if truncated {
        GltfAddressabilityProjectionV2::unavailable(vec![
            GltfAddressabilityUnavailableReasonV2::TargetDomainTruncated,
        ])
    } else {
        GltfAddressabilityProjectionV2::Available { value: () }
    };
    Ok((coverage, targets))
}

fn target_base_reasons(
    raw: &RawGltfAddressabilityInventoryV1,
    animations: &GltfAnimationAddressabilityInventoryV1,
    bevy_animation_enabled: bool,
    load_animations: bool,
    pointer_width: Option<TargetPointerWidth>,
) -> Vec<GltfAddressabilityUnavailableReasonV2> {
    let mut reasons = Vec::new();
    if !raw.node_coverage().is_complete()
        || !raw.scene_coverage().is_complete()
        || !raw.path_candidate_coverage().is_complete()
        || !matches!(
            animations.animations().coverage().state(),
            crate::GltfAnimationCoverageStateV1::Complete
        )
        || animations.animations().rows().iter().any(|animation| {
            !matches!(
                animation.channels().coverage().state(),
                crate::GltfAnimationCoverageStateV1::Complete
            )
        })
    {
        reasons.push(GltfAddressabilityUnavailableReasonV2::RawSourceIncomplete);
    }
    if !raw.dependency_closure().coverage().is_complete() {
        reasons.push(GltfAddressabilityUnavailableReasonV2::DependencyClosureIncomplete);
    }
    if !bevy_animation_enabled {
        reasons.push(GltfAddressabilityUnavailableReasonV2::BevyAnimationFeatureDisabled);
    }
    if !load_animations {
        reasons.push(GltfAddressabilityUnavailableReasonV2::LoadAnimationsDisabled);
    }
    if pointer_width.is_none() {
        reasons.push(GltfAddressabilityUnavailableReasonV2::TargetPointerWidthMissing);
    }
    reasons
}

fn invalidate_target_collisions(targets: &mut [GltfAddressabilityTargetV2]) {
    let mut paths = BTreeMap::<Vec<String>, Vec<usize>>::new();
    let mut uuids = BTreeMap::<String, Vec<usize>>::new();
    for (index, target) in targets.iter().enumerate() {
        if let GltfAddressabilityProjectionV2::Available { value } = &target.projection {
            paths.entry(value.segments.clone()).or_default().push(index);
            uuids.entry(value.uuid.clone()).or_default().push(index);
        }
    }
    let duplicate_paths = paths
        .into_values()
        .filter(|indices| indices.len() > 1)
        .flatten()
        .collect::<BTreeSet<_>>();
    let uuid_collisions = uuids
        .into_values()
        .filter(|indices| indices.len() > 1)
        .flatten()
        .collect::<BTreeSet<_>>();
    for (index, target) in targets.iter_mut().enumerate() {
        let mut reasons = Vec::new();
        if duplicate_paths.contains(&index) {
            reasons.push(GltfAddressabilityUnavailableReasonV2::DuplicateFullPath);
        }
        if uuid_collisions.contains(&index) {
            reasons.push(GltfAddressabilityUnavailableReasonV2::TargetIdCollision);
        }
        if !reasons.is_empty() {
            target.projection = GltfAddressabilityProjectionV2::unavailable(reasons);
        }
    }
}

/// Invalid rich glTF addressability input or adapter evidence.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum GltfAddressabilityV2Error {
    /// The rich report or inventory schema/header is not immutable V2.
    #[error("invalid rich glTF addressability schema or header")]
    InvalidSchema,
    /// The raw sidecar did not pass its own strict validation.
    #[error("invalid raw glTF addressability inventory")]
    InvalidRawInventory,
    /// Raw and animation inventories do not identify the same primary/closure.
    #[error("raw and animation addressability inventories are not from the same load")]
    InventoryBindingMismatch,
    /// The selected profile is not exact Bevy 0.19.0 revision 3.
    #[error("rich addressability requires exact Bevy 0.19.0 profile revision 3")]
    InvalidBevyProfile,
    /// One of the two required Bevy animation settings was not materialized.
    #[error("rich addressability is missing a required Bevy setting")]
    MissingRequiredSetting,
    /// Prediction provenance is invalid or does not bind this exact inventory.
    #[error("invalid rich addressability prediction provenance")]
    InvalidPredictionProvenance,
    /// The separately versioned Bevy rule bundle was mutated.
    #[error("invalid Bevy glTF addressability rule bundle")]
    InvalidRuleBundle,
    /// The real check evaluation does not correspond one-to-one to projections.
    #[error("invalid rich engine-addressability check evaluation")]
    InvalidCheckEvaluation,
}

#[cfg(test)]
mod tests {
    use super::*;
    use animsmith_core::{
        DependencyClosureBuilderV1, Document, RawGltfAddressabilityCoverageV1,
        RawGltfAddressabilityInventoryInputV1, RawGltfInverseBindMatricesObservationV1,
        RawGltfNodeRowV1, RawGltfScenePathCandidateRowV1, RawGltfSceneRowV1,
        RawGltfSkinAttachmentRowV1, RawGltfSkinRowV1, RawSourceFactsBuilderV1, SourceFactDomainV1,
        SourceFormatV1, ToolSource,
    };
    use std::io::Cursor;

    fn empty_loaded_source() -> LoadedSource {
        let primary = InputIdentity::from_bytes(b"rich-addressability");
        let mut facts = RawSourceFactsBuilderV1::new(SourceFormatV1::GltfJson, primary.clone());
        facts.mark_complete(SourceFactDomainV1::Clips);
        let closure = DependencyClosureBuilderV1::new(
            primary,
            facts.resource_coverage(),
            facts.resource_rows().len(),
        )
        .finish()
        .unwrap();
        facts
            .finish_with_dependency_closure(Document::default(), closure)
            .unwrap()
    }

    fn analytic_raw(source: &LoadedSource) -> RawGltfAddressabilityInventoryV1 {
        RawGltfAddressabilityInventoryV1::new(
            source.source_facts().primary_identity().clone(),
            source.dependency_closure().clone(),
            RawGltfAddressabilityInventoryInputV1 {
                default_scene: RawGltfDefaultSceneObservationV1::Selected {
                    source_scene_index: 0,
                },
                scene_coverage: RawGltfAddressabilityCoverageV1::Complete,
                scenes: vec![
                    RawGltfSceneRowV1::new(0, Some("World".into()), vec![0]),
                    RawGltfSceneRowV1::new(1, Some("World".into()), vec![0]),
                ],
                node_coverage: RawGltfAddressabilityCoverageV1::Complete,
                nodes: vec![RawGltfNodeRowV1::new(0, None, None, Vec::new())],
                skin_coverage: RawGltfAddressabilityCoverageV1::Complete,
                skins: vec![
                    RawGltfSkinRowV1::new(
                        0,
                        Some("Rig".into()),
                        vec![0],
                        Some(0),
                        RawGltfInverseBindMatricesObservationV1::Absent,
                    ),
                    RawGltfSkinRowV1::new(
                        1,
                        Some("Rig".into()),
                        vec![0],
                        None,
                        RawGltfInverseBindMatricesObservationV1::Absent,
                    ),
                ],
                attachment_coverage: RawGltfAddressabilityCoverageV1::Complete,
                attachments: vec![RawGltfSkinAttachmentRowV1::new(0, 0)],
                path_candidate_coverage: RawGltfAddressabilityCoverageV1::Complete,
                path_candidates: vec![
                    RawGltfScenePathCandidateRowV1::new(0, 0, vec![0]),
                    RawGltfScenePathCandidateRowV1::new(1, 1, vec![0]),
                ],
            },
        )
        .unwrap()
    }

    #[test]
    fn frozen_rule_bundle_is_strict_and_commit_pinned() {
        for width in [
            None,
            Some(TargetPointerWidth::Bits32),
            Some(TargetPointerWidth::Bits64),
        ] {
            let bundle = BevyGltfAddressabilityRulesV1::frozen(width);
            assert!(bundle.validate());
            assert_eq!(bundle.profile_revision(), 3);
            assert_eq!(bundle.sources().len(), 6);
            assert!(
                bundle
                    .sources()
                    .windows(2)
                    .all(|pair| pair[0].id() < pair[1].id())
            );
            assert!(
                bundle
                    .sources()
                    .iter()
                    .all(|source| source.url().contains(BEVY_COMMIT))
            );
        }
    }

    #[test]
    fn target_id_golden_vectors_cover_both_pointer_widths_and_boundaries() {
        // Goldens are independently obtained from Bevy 0.19.0's exact
        // `AnimationTargetId::from_iter` implementation on 32- and 64-bit
        // targets.  They intentionally differ because Bevy hashes `usize`.
        let paths: &[&[&str]] = &[
            &[],
            &["Root"],
            &["Root", "Hips", "GltfNode7"],
            &["ab", "c"],
            &["a", "bc"],
        ];
        let expected_32 = [
            "77da024c-dd93-5d43-8f9a-2af3d4b17c5b",
            "2514296d-dbd8-5ea8-a8cc-1fadaf961d2f",
            "15c3912b-7127-55c1-b5ff-83950e3d0f9a",
            "fd324118-d34f-53d0-9787-d5a1594985e7",
            "2ceb26c6-8d62-53c7-b4c5-ee3b9b81784e",
        ];
        let expected_64 = [
            "77da024c-dd93-5d43-8f9a-2af3d4b17c5b",
            "05c24be8-bacd-5ee9-9a65-f8215983900e",
            "90fed833-c803-5800-883d-adee90aab446",
            "0d54b499-ea4e-5a49-afec-0a87e56da473",
            "c22eb6e6-9f59-509b-9cfc-79d424c0d445",
        ];
        for ((path, expected_32), expected_64) in paths.iter().zip(expected_32).zip(expected_64) {
            assert_eq!(
                bevy_animation_target_id_v1(path.iter().copied(), TargetPointerWidth::Bits32)
                    .unwrap(),
                expected_32
            );
            assert_eq!(
                bevy_animation_target_id_v1(path.iter().copied(), TargetPointerWidth::Bits64)
                    .unwrap(),
                expected_64
            );
        }

        let max_segment = "x".repeat(GLTF_ADDRESSABILITY_V2_MAX_SEGMENT_BYTES);
        assert!(
            bevy_animation_target_id_v1([max_segment.as_str()], TargetPointerWidth::Bits64).is_ok()
        );
        let over_segment = format!("{max_segment}x");
        assert!(matches!(
            bevy_animation_target_id_v1([over_segment.as_str()], TargetPointerWidth::Bits64),
            Err(BevyAnimationTargetIdError::SegmentTooLong { .. })
        ));
    }

    #[test]
    fn analytic_labels_default_route_and_named_winners_follow_bevy_rules() {
        let source = empty_loaded_source();
        let raw = analytic_raw(&source);
        let animations = GltfAnimationAddressabilityInventoryV1::from_source(&source).unwrap();
        let projection = BevyGltfAddressabilityProjectionV2::from_inventories(
            &raw,
            &animations,
            true,
            true,
            Some(TargetPointerWidth::Bits64),
        )
        .unwrap();
        assert_eq!(
            projection
                .scenes()
                .iter()
                .map(GltfAddressabilitySceneV2::label)
                .collect::<Vec<_>>(),
            ["Scene0", "Scene1"]
        );
        assert_eq!(
            projection.default_scene_route(),
            &GltfAddressabilityProjectionV2::Available {
                value: "Scene0".into()
            }
        );
        assert!(matches!(
            projection.skins()[0].skin_label(),
            GltfAddressabilityProjectionV2::Available { value } if value == "Skin0"
        ));
        assert!(matches!(
            projection.skins()[1].skin_label(),
            GltfAddressabilityProjectionV2::ProvenAbsent
        ));
        assert_eq!(
            projection.skins()[1].inverse_bind_matrices_label(),
            "Skin1/InverseBindMatrices"
        );
        let scene_map = &projection.named_maps()[0];
        assert!(matches!(
            &scene_map.winners,
            GltfAddressabilityProjectionV2::Available { value }
                if value.len() == 1 && value[0].source_index() == 1
        ));
        let skin_map = &projection.named_maps()[2];
        assert!(matches!(
            &skin_map.winners,
            GltfAddressabilityProjectionV2::Available { value }
                if value.len() == 1 && value[0].source_index() == 0
        ));
    }

    #[test]
    fn retained_default_scene_is_exact_under_partial_tail_coverage() {
        let source = empty_loaded_source();
        let raw = RawGltfAddressabilityInventoryV1::new(
            source.source_facts().primary_identity().clone(),
            source.dependency_closure().clone(),
            RawGltfAddressabilityInventoryInputV1 {
                default_scene: RawGltfDefaultSceneObservationV1::Selected {
                    source_scene_index: 0,
                },
                scene_coverage: RawGltfAddressabilityCoverageV1::budget_exceeded(),
                scenes: vec![RawGltfSceneRowV1::new(0, None, Vec::new())],
                node_coverage: RawGltfAddressabilityCoverageV1::Complete,
                nodes: Vec::new(),
                skin_coverage: RawGltfAddressabilityCoverageV1::Complete,
                skins: Vec::new(),
                attachment_coverage: RawGltfAddressabilityCoverageV1::Complete,
                attachments: Vec::new(),
                path_candidate_coverage: RawGltfAddressabilityCoverageV1::Complete,
                path_candidates: Vec::new(),
            },
        )
        .unwrap();
        let animations = GltfAnimationAddressabilityInventoryV1::from_source(&source).unwrap();
        let projection = BevyGltfAddressabilityProjectionV2::from_inventories(
            &raw,
            &animations,
            true,
            true,
            Some(TargetPointerWidth::Bits64),
        )
        .unwrap();
        assert_eq!(
            projection.default_scene_route(),
            &GltfAddressabilityProjectionV2::Available {
                value: "Scene0".into()
            }
        );
    }

    #[test]
    fn named_map_keys_enforce_exact_segment_bound() {
        let winner = |name: String| {
            let mut winners = BTreeMap::new();
            winners.insert(
                name.clone(),
                GltfAddressabilityNamedMapWinnerV2 {
                    name,
                    source_index: 0,
                    typed_label: "Scene0".into(),
                },
            );
            named_map(GltfAddressabilityNamedMapKindV2::Scene, true, winners)
        };
        assert!(matches!(
            winner("x".repeat(GLTF_ADDRESSABILITY_V2_MAX_SEGMENT_BYTES)).winners(),
            GltfAddressabilityProjectionV2::Available { .. }
        ));
        assert!(matches!(
            winner("x".repeat(GLTF_ADDRESSABILITY_V2_MAX_SEGMENT_BYTES + 1)).winners(),
            GltfAddressabilityProjectionV2::RequiredUnavailable { reasons }
                if reasons == &[GltfAddressabilityUnavailableReasonV2::ProjectionBoundsExceeded]
        ));
    }

    #[test]
    fn slash_joined_display_paths_do_not_alias_segment_vectors() {
        let target = |source_node_index, segments: Vec<&str>, width| {
            let owned = segments
                .iter()
                .map(|segment| (*segment).to_owned())
                .collect::<Vec<_>>();
            GltfAddressabilityTargetV2 {
                source_node_index,
                contributing_channels: vec![GltfAddressabilityTargetChannelV2 {
                    source_animation_index: 0,
                    source_channel_index: source_node_index,
                }],
                projection: GltfAddressabilityProjectionV2::Available {
                    value: GltfAddressabilityTargetValueV2 {
                        path: owned.join("/"),
                        uuid: bevy_animation_target_id_v1(segments, width).unwrap(),
                        segments: owned,
                    },
                },
            }
        };
        let mut targets = vec![
            target(0, vec!["a/b", "c"], TargetPointerWidth::Bits64),
            target(1, vec!["a", "b/c"], TargetPointerWidth::Bits64),
        ];
        invalidate_target_collisions(&mut targets);
        assert!(targets.iter().all(|target| matches!(
            target.projection(),
            GltfAddressabilityProjectionV2::Available { .. }
        )));
    }

    #[test]
    fn standalone_inventory_round_trip_is_strict_and_identity_bound() {
        let source = empty_loaded_source();
        let raw = analytic_raw(&source);
        let animations = GltfAnimationAddressabilityInventoryV1::from_source(&source).unwrap();
        let report = GltfAddressabilityV2::new(
            ToolInfo::animsmith(ToolSource::new(None, None)),
            raw,
            animations,
            None,
        )
        .unwrap();
        let json = serde_json::to_value(&report).unwrap();
        let readback =
            GltfAddressabilityV2::read_from(Cursor::new(serde_json::to_vec(&json).unwrap()))
                .unwrap();
        assert_eq!(readback.input(), report.input());
        assert_eq!(readback.inventory(), report.inventory());
        assert!(readback.bevy().is_none());

        let mut wrong_schema = json.clone();
        wrong_schema["schema"] = serde_json::json!("urn:animsmith:schema:gltf-addressability:1");
        assert!(
            GltfAddressabilityV2::read_from(Cursor::new(
                serde_json::to_vec(&wrong_schema).unwrap()
            ))
            .is_err()
        );

        let mut wrong_binding = json;
        wrong_binding["input"]["bytes"] = serde_json::json!(999);
        assert!(
            GltfAddressabilityV2::read_from(Cursor::new(
                serde_json::to_vec(&wrong_binding).unwrap()
            ))
            .is_err()
        );
    }
}
