//! Typed configuration: rig selection, per-check settings, per-clip
//! expectations and movement ownership, and typed clip groups. The TOML file (`animsmith.toml`) is
//! *one* constructor of this — embedding pipelines build it
//! programmatically through this module and keep their own contract
//! formats on their side.
//!
//! The structs derive `Deserialize` so a frontend can parse any
//! serde-compatible format (the CLI uses TOML); the core itself never
//! touches a file format. [`crate::CheckCtx::new`] does not resolve
//! [`Config::rig`]; the embedding frontend resolves roles first through
//! [`crate::profile`] and passes the resulting [`crate::ResolvedRoles`].

use crate::finding::Severity;
use crate::metrics::MIN_STRIDE_STEP_M;
use crate::profile::Role;
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;

/// A pinned expectation: declared value ± tolerance.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Pinned {
    /// Expected value.
    pub value: f64,
    /// Allowed absolute deviation from [`Pinned::value`].
    pub tolerance: f64,
}

/// Severity override for a check; `Off` disables it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SeveritySetting {
    /// Remove the check from the run set.
    Off,
    /// Force content findings to notes.
    Note,
    /// Force content findings to warnings.
    #[serde(alias = "warning")]
    Warn,
    /// Force content findings to errors.
    Error,
}

/// The system that owns one component of a clip's world movement.
///
/// This is project intent, not a fact inferred from the animation or an
/// engine profile. [`MovementOwner::Gameplay`] means the entity/controller
/// supplies the component and an importer should bake it into pose;
/// [`MovementOwner::Animation`] means extracted root motion supplies it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MovementOwner {
    /// The entity or gameplay controller owns this movement component.
    Gameplay,
    /// Extracted animation root motion owns this movement component.
    Animation,
}

impl MovementOwner {
    /// Convert the legacy horizontal `in_place` declaration into its canonical
    /// movement owner.
    pub const fn from_in_place(in_place: bool) -> Self {
        if in_place {
            Self::Gameplay
        } else {
            Self::Animation
        }
    }
}

impl SeveritySetting {
    /// Convert this setting into a finding severity.
    ///
    /// Returns `None` for [`SeveritySetting::Off`] because disabling a
    /// check is handled before execution.
    pub fn as_severity(self) -> Option<Severity> {
        match self {
            SeveritySetting::Off => None,
            SeveritySetting::Note => Some(Severity::Note),
            SeveritySetting::Warn => Some(Severity::Warning),
            SeveritySetting::Error => Some(Severity::Error),
        }
    }
}

/// Per-check settings: a severity override plus the union of the
/// built-in checks' tunables (only the owning check reads each field).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckSettings {
    /// Per-check severity override. `None` leaves the check's default
    /// severity intact.
    pub severity: Option<SeveritySetting>,
    /// `loop-seam`: finite non-negative ratio above which the seam is a pop
    /// (default 1.5).
    #[serde(default, deserialize_with = "deserialize_nonnegative_finite_option")]
    pub max_ratio: Option<f64>,
    /// `loop-seam`: finite non-negative stride floor in metres (default 0.02).
    #[serde(default, deserialize_with = "deserialize_nonnegative_finite_option")]
    pub min_stride_step_m: Option<f64>,
    /// `loop-closure`: finite non-negative maximum model-space position delta
    /// in metres (default 0.01).
    #[serde(default, deserialize_with = "deserialize_nonnegative_finite_option")]
    pub max_position_delta_m: Option<f64>,
    /// `loop-closure`: finite non-negative maximum model-space rotation delta
    /// in degrees (default 1.0).
    #[serde(default, deserialize_with = "deserialize_nonnegative_finite_option")]
    pub max_rotation_delta_deg: Option<f64>,
    /// `loop-seam-vel`: finite non-negative maximum incoming/outgoing
    /// model-space linear-velocity difference in metres per second (default
    /// 0.1).
    #[serde(default, deserialize_with = "deserialize_nonnegative_finite_option")]
    pub max_velocity_delta_mps: Option<f64>,
    /// `loop-seam-rot`: finite non-negative maximum incoming/outgoing
    /// model-space angular-velocity difference in degrees per second (default
    /// 5.0).
    #[serde(default, deserialize_with = "deserialize_nonnegative_finite_option")]
    pub max_angular_velocity_delta_degps: Option<f64>,
    /// `frozen-bone`: finite non-negative rotation floor in degrees (default
    /// 1.0).
    #[serde(default, deserialize_with = "deserialize_nonnegative_finite_option")]
    pub min_rotation_deg: Option<f64>,
    /// `bind-pose`: finite non-negative mean first-frame deviation cap in
    /// degrees (default 45).
    #[serde(default, deserialize_with = "deserialize_nonnegative_finite_option")]
    pub max_mean_rest_delta_deg: Option<f64>,
    /// `foot-slide`: finite non-negative contact height above the per-clip foot
    /// minimum (default 0.03 m).
    #[serde(default, deserialize_with = "deserialize_nonnegative_finite_option")]
    pub contact_height_m: Option<f64>,
    /// `foot-slide`: finite non-negative allowed stance-speed deviation
    /// (default 0.3 m/s).
    #[serde(default, deserialize_with = "deserialize_nonnegative_finite_option")]
    pub max_slide_mps: Option<f64>,
    /// Compatibility alias for [`RuntimeNodesConfig::selectors`].
    ///
    /// `rest-world-scale` consumes the shared runtime-node authority. This
    /// legacy field remains accepted for existing configuration only when the
    /// shared selector field is absent; declaring both fields is rejected by
    /// [`Config::validate`].
    pub node_selectors: Option<Vec<String>>,
    /// `rest-world-scale`: expected positive uniform scale factor (default
    /// 1.0).
    #[serde(default, deserialize_with = "deserialize_positive_finite_option")]
    pub expected_uniform_scale: Option<f64>,
    /// `rest-world-scale`: inclusive absolute tolerance around the expected
    /// uniform factor (default 0.0001).
    #[serde(default, deserialize_with = "deserialize_nonnegative_finite_option")]
    pub uniform_scale_tolerance: Option<f64>,
}

/// What the author declares about one clip (or a glob of clips).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipExpectations {
    /// The clip is a cyclic loop; loop checks apply.
    #[serde(rename = "loop")]
    pub looping: Option<bool>,
    /// `loop-closure`: per-clip maximum model-space position delta in
    /// metres. When unset, the global `loop-closure` setting (or its
    /// built-in default) applies.
    #[serde(default, deserialize_with = "deserialize_nonnegative_finite_option")]
    pub max_loop_position_delta_m: Option<f64>,
    /// `loop-closure`: per-clip maximum model-space rotation delta in
    /// degrees. When unset, the global `loop-closure` setting (or its
    /// built-in default) applies.
    #[serde(default, deserialize_with = "deserialize_nonnegative_finite_option")]
    pub max_loop_rotation_delta_deg: Option<f64>,
    /// `loop-seam-vel`: per-clip maximum incoming/outgoing model-space
    /// linear-velocity difference in metres per second. When unset, the
    /// global `loop-seam-vel` setting (or its built-in default) applies.
    #[serde(default, deserialize_with = "deserialize_nonnegative_finite_option")]
    pub max_loop_velocity_delta_mps: Option<f64>,
    /// `loop-seam-rot`: per-clip maximum incoming/outgoing model-space
    /// angular-velocity difference in degrees per second. When unset, the
    /// global `loop-seam-rot` setting (or its built-in default) applies.
    #[serde(default, deserialize_with = "deserialize_nonnegative_finite_option")]
    pub max_loop_angular_velocity_delta_degps: Option<f64>,
    /// Expected clip duration in seconds; consumed by the
    /// `duration-sanity` check. Its value must be finite and positive,
    /// and its tolerance must be finite and non-negative.
    pub duration_s: Option<Pinned>,
    /// Declared locomotion speed (m/s) carried by the clip's root
    /// motion.
    pub speed_mps: Option<Pinned>,
    /// Owner of horizontal X/Z world movement.
    pub movement_owner_xz: Option<MovementOwner>,
    /// Owner of vertical Y world movement.
    pub movement_owner_y: Option<MovementOwner>,
    /// Owner of world yaw movement.
    pub movement_owner_yaw: Option<MovementOwner>,
    /// Compatibility input alias for [`ClipExpectations::movement_owner_xz`]:
    /// `true` means [`MovementOwner::Gameplay`] and `false` means
    /// [`MovementOwner::Animation`]. A selector entry must not declare both
    /// spellings. Effective expectations returned by
    /// [`Config::expectations_for`] normalize this alias into
    /// `movement_owner_xz` and clear this field.
    pub in_place: Option<bool>,
    /// Authored frame rate; consumed by the `fps` check (keys must land
    /// on the `1/fps` grid).
    pub fps: Option<f64>,
    /// Bones that must carry keyframes and actually move
    /// (`missing-bones` presence + `frozen-bone` rotation floor).
    pub animates_bones: Option<Vec<String>>,
}

/// Effective per-clip tolerances for loop pose and seam velocity.
///
/// Obtain this value through [`Config::loop_continuity_tolerances`]. It
/// resolves the same exact-name/glob precedence and per-check defaults used by
/// the built-in loop checks, so artifact proofs do not need a second tolerance
/// authority.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoopContinuityTolerances {
    max_position_delta_m: f64,
    max_rotation_delta_deg: f64,
    max_velocity_delta_mps: f64,
    max_angular_velocity_delta_degps: f64,
}

impl LoopContinuityTolerances {
    /// Inclusive model-space endpoint-position tolerance in metres.
    pub const fn max_position_delta_m(self) -> f64 {
        self.max_position_delta_m
    }

    /// Inclusive shortest-path endpoint-rotation tolerance in degrees.
    pub const fn max_rotation_delta_deg(self) -> f64 {
        self.max_rotation_delta_deg
    }

    /// Inclusive incoming/outgoing linear-velocity tolerance in metres per second.
    pub const fn max_velocity_delta_mps(self) -> f64 {
        self.max_velocity_delta_mps
    }

    /// Inclusive incoming/outgoing angular-velocity tolerance in degrees per second.
    pub const fn max_angular_velocity_delta_degps(self) -> f64 {
        self.max_angular_velocity_delta_degps
    }
}

impl ClipExpectations {
    /// Canonical horizontal owner declared by this selector entry.
    ///
    /// Call [`Config::validate`] before resolving expectations so a selector
    /// that declares both the canonical field and its compatibility alias is
    /// rejected as a typed configuration error.
    pub fn normalized_movement_owner_xz(&self) -> Option<MovementOwner> {
        self.movement_owner_xz
            .or_else(|| self.in_place.map(MovementOwner::from_in_place))
    }

    /// Overlay `other` on `self` (other's set fields win).
    fn merged_with(&self, other: &ClipExpectations) -> ClipExpectations {
        ClipExpectations {
            looping: other.looping.or(self.looping),
            max_loop_position_delta_m: other
                .max_loop_position_delta_m
                .or(self.max_loop_position_delta_m),
            max_loop_rotation_delta_deg: other
                .max_loop_rotation_delta_deg
                .or(self.max_loop_rotation_delta_deg),
            max_loop_velocity_delta_mps: other
                .max_loop_velocity_delta_mps
                .or(self.max_loop_velocity_delta_mps),
            max_loop_angular_velocity_delta_degps: other
                .max_loop_angular_velocity_delta_degps
                .or(self.max_loop_angular_velocity_delta_degps),
            duration_s: other.duration_s.or(self.duration_s),
            speed_mps: other.speed_mps.or(self.speed_mps),
            movement_owner_xz: other
                .normalized_movement_owner_xz()
                .or_else(|| self.normalized_movement_owner_xz()),
            movement_owner_y: other.movement_owner_y.or(self.movement_owner_y),
            movement_owner_yaw: other.movement_owner_yaw.or(self.movement_owner_yaw),
            in_place: None,
            fps: other.fps.or(self.fps),
            animates_bones: other
                .animates_bones
                .clone()
                .or_else(|| self.animates_bones.clone()),
        }
    }
}

/// Deserialize an optional non-negative finite cap.
///
/// Loop-continuity evidence is always non-negative and finite, so accepting a
/// negative or non-finite cap would make its pass/fail result surprising.
fn deserialize_nonnegative_finite_option<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<f64>::deserialize(deserializer)?;
    if let Some(value) = value
        && !is_nonnegative_finite(value)
    {
        return Err(serde::de::Error::custom(
            "must be a finite non-negative number",
        ));
    }
    Ok(value)
}

fn deserialize_positive_finite_option<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<f64>::deserialize(deserializer)?;
    if value.is_some_and(|value| !is_positive_finite(value)) {
        return Err(serde::de::Error::custom(
            "must be a finite number greater than zero",
        ));
    }
    Ok(value)
}

fn is_nonnegative_finite(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

fn is_positive_finite(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

/// A set of clips whose gait phases must agree (a directional blend
/// ring).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GaitGroup {
    /// Clip names that should share a gait phase.
    pub clips: Vec<String>,
    /// Maximum circular spread of the members' gait phases, in cycle
    /// fraction `[0, 0.5]`.
    pub max_gait_phase_spread: f64,
    /// Members with L−R amplitude under this (metres) are excluded from
    /// the spread (their phase is noise, not signal).
    #[serde(default)]
    pub min_lr_amplitude_m: f64,
}

/// Thresholds for detecting a pair that is more phase-similar under reflected
/// time than under a declared same-time / absolute-sync rule.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeComplementSettings {
    /// Minimum reflected-time minus same-time phase-similarity score required
    /// to report the diagnostic. This threshold is in `[0, 1]`; emitted
    /// advantages are positive and no greater than one.
    #[serde(deserialize_with = "deserialize_unit_interval")]
    pub min_reflected_time_advantage: f64,
    /// Minimum L−R foot-height amplitude (metres) required before a phase is
    /// considered evidence rather than noise.
    #[serde(deserialize_with = "deserialize_nonnegative_finite")]
    pub min_lr_amplitude_m: f64,
}

fn deserialize_unit_interval<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    if !is_unit_interval(value) {
        return Err(serde::de::Error::custom(
            "must be a finite number in the range [0, 1]",
        ));
    }
    Ok(value)
}

fn deserialize_nonnegative_finite<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f64::deserialize(deserializer)?;
    if !is_nonnegative_finite(value) {
        return Err(serde::de::Error::custom(
            "must be a finite non-negative number",
        ));
    }
    Ok(value)
}

fn is_unit_interval(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

/// A set of clips sampled together by a same-time / absolute-sync runtime.
///
/// The group compares timing representations; it does not prescribe a runtime
/// retiming or repair strategy.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncGroup {
    /// Clip names that must be compatible when sampled at the same time.
    pub clips: Vec<String>,
    /// Largest permitted duration range across members, in seconds.
    pub max_duration_delta_s: f64,
    /// Largest permitted longest-channel key-count range across members.
    pub max_frame_count_delta: u32,
    /// Largest permitted declared frame-rate range across members.
    pub max_fps_delta: f64,
    /// Optional phase-similarity diagnostic for time-complementary member
    /// pairs. A two-member group declares one pair; larger groups compare each
    /// configured-order unordered pair.
    #[serde(default)]
    pub time_complement: Option<TimeComplementSettings>,
}

/// Rig selection: a named profile ("auto" to detect) and/or an inline
/// role map (which wins over the profile for the roles it names).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigConfig {
    /// Built-in profile name, or `"auto"` to select the best built-in
    /// match.
    #[serde(default = "default_profile")]
    pub profile: String,
    /// Inline role-to-bone-name bindings. These are interpreted as
    /// explicit overrides by callers that merge them with a profile.
    #[serde(default)]
    pub roles: BTreeMap<Role, String>,
    /// Bone names that must be present in the file's skeleton, regardless of
    /// whether any clip keys them. This is for static runtime sockets, IK
    /// targets, and mask bones; use [`ClipExpectations::animates_bones`] when
    /// a bone must carry animation data in a particular clip.
    pub required_bones: Option<Vec<String>>,
}

fn default_profile() -> String {
    "auto".into()
}

impl Default for RigConfig {
    fn default() -> Self {
        Self {
            profile: default_profile(),
            roles: BTreeMap::new(),
            required_bones: None,
        }
    }
}

/// Engine-neutral runtime-node selection policy.
///
/// A runtime node is a source node whose identity matters to a consuming
/// runtime, such as an attachment socket or IK target. The policy intentionally
/// says nothing about a particular engine or the operation consuming it. An
/// absent [`Self::selectors`] field and an explicit empty list both mean no
/// runtime-node policy is declared.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeNodesConfig {
    /// Exact source-node names or `*` globs, in declared priority order.
    ///
    /// Duplicate selectors are accepted and deterministically de-duplicated
    /// by [`Config::runtime_node_selectors`], retaining their first occurrence.
    #[serde(default)]
    pub selectors: Option<Vec<String>>,
}

/// Normalized, deterministic runtime-node selection authority.
///
/// Obtain this value through [`Config::runtime_node_selectors`] after calling
/// [`Config::validate`]. It retains configured selector order while removing
/// later duplicate spellings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeNodeSelectors {
    selectors: Vec<String>,
}

impl RuntimeNodeSelectors {
    fn new(selectors: &[String]) -> Self {
        let mut seen = std::collections::BTreeSet::new();
        Self {
            selectors: selectors
                .iter()
                .filter(|selector| seen.insert(selector.as_str()))
                .cloned()
                .collect(),
        }
    }

    /// Normalized selectors in configured first-occurrence order.
    pub fn selectors(&self) -> &[String] {
        &self.selectors
    }

    /// Resolve every selector against named candidates in deterministic input
    /// order.
    ///
    /// Exact names and `*` globs follow [`glob_match`]. Every configured
    /// selector receives a result so consumers can preserve distinct no-match
    /// and ambiguity handling without duplicating selector semantics. Unnamed
    /// candidates are represented by omitting them from `candidates`.
    pub fn resolve<'a, T: Clone>(
        &self,
        candidates: impl IntoIterator<Item = (&'a str, T)>,
    ) -> Vec<RuntimeNodeSelectorResolution<T>> {
        let candidates = candidates.into_iter().collect::<Vec<_>>();
        self.selectors
            .iter()
            .map(|selector| {
                let matches = candidates
                    .iter()
                    .filter(|(name, _)| glob_match(selector, name))
                    .map(|(_, candidate)| candidate.clone())
                    .collect::<Vec<_>>();
                match matches.as_slice() {
                    [] => RuntimeNodeSelectorResolution::NoMatch {
                        selector: selector.clone(),
                    },
                    [node] => RuntimeNodeSelectorResolution::ExactlyOne {
                        selector: selector.clone(),
                        node: node.clone(),
                    },
                    _ => RuntimeNodeSelectorResolution::Ambiguous {
                        selector: selector.clone(),
                        nodes: matches,
                    },
                }
            })
            .collect()
    }
}

/// One runtime-node selector's deterministic resolution result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeNodeSelectorResolution<T> {
    /// The selector matched no named candidate.
    NoMatch {
        /// Configured selector spelling.
        selector: String,
    },
    /// The selector resolved to exactly one candidate.
    ExactlyOne {
        /// Configured selector spelling.
        selector: String,
        /// The sole matching candidate.
        node: T,
    },
    /// The selector matched more than one candidate.
    Ambiguous {
        /// Configured selector spelling.
        selector: String,
        /// Matching candidates in the input's deterministic order.
        nodes: Vec<T>,
    },
}

/// Invalid values in a directly constructed [`Config`].
///
/// Numeric values are intentionally not retained in this error so it remains
/// equality-comparable even when the rejected input was `NaN`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigValidationError {
    /// A numeric per-check policy setting was outside its documented domain.
    #[error("check {check_id:?} field {field:?} must be finite and within its documented range")]
    InvalidCheckSetting {
        /// Configured stable check id.
        check_id: String,
        /// Stable public [`CheckSettings`] field name.
        field: &'static str,
    },
    /// A per-clip loop-continuity cap was negative or non-finite.
    #[error("clip selector {selector:?} field {field:?} must be a finite non-negative number")]
    InvalidClipLoopCap {
        /// Exact clip name or glob containing the invalid cap.
        selector: String,
        /// Stable public [`ClipExpectations`] field name.
        field: &'static str,
    },
    /// One clip selector declared both the canonical horizontal owner and its
    /// legacy `in_place` alias.
    #[error(
        "clip selector {selector:?} cannot declare both \"movement_owner_xz\" and \"in_place\""
    )]
    ConflictingClipMovementOwner {
        /// Exact clip name or glob containing both spellings.
        selector: String,
    },
    /// The shared runtime-node selector field and its rest-world-scale
    /// compatibility alias were both declared.
    #[error(
        "cannot declare both \"runtime_nodes.selectors\" and \"checks.rest-world-scale.node_selectors\""
    )]
    ConflictingRuntimeNodeSelectors,
    /// A sync-group tolerance was negative or non-finite.
    #[error("sync group {group:?} field {field:?} must be a finite non-negative number")]
    InvalidSyncGroupTolerance {
        /// Configured group name.
        group: String,
        /// Stable public field name.
        field: &'static str,
    },
    /// A time-complement setting was outside its finite declared domain.
    #[error(
        "sync group {group:?} time-complement field {field:?} must be finite and within its documented range"
    )]
    InvalidTimeComplementSetting {
        /// Configured group name.
        group: String,
        /// Stable public field name.
        field: &'static str,
    },
}

/// The whole configuration. Field names match the `animsmith.toml`
/// sections.
///
/// [`Self::runtime_nodes`] is an intentional pre-1.0 additive public field.
/// Embedders that construct this struct with a literal must add
/// `runtime_nodes: RuntimeNodesConfig::default()` or use
/// `..Config::default()`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Declarative rig profile and inline role bindings. Frontends resolve
    /// these into [`crate::ResolvedRoles`] before creating a check context;
    /// the core runner does not apply them automatically.
    #[serde(default)]
    pub rig: RigConfig,
    /// Per-check settings keyed by stable check id.
    #[serde(default)]
    pub checks: BTreeMap<String, CheckSettings>,
    /// Shared engine-neutral policy for source nodes addressed by the runtime.
    #[serde(default)]
    pub runtime_nodes: RuntimeNodesConfig,
    /// Keyed by clip name or glob (`*` wildcards). An exact-name entry
    /// overrides glob entries; among globs, later (lexicographically
    /// greater) keys win on conflict.
    #[serde(default)]
    pub clips: BTreeMap<String, ClipExpectations>,
    /// Named gait groups consumed by the `gait-group` check.
    #[serde(default)]
    pub gait_groups: BTreeMap<String, GaitGroup>,
    /// Named same-time / absolute-sync groups consumed by `sync-group`.
    #[serde(default)]
    pub sync_groups: BTreeMap<String, SyncGroup>,
}

impl Config {
    /// Validate values that can also be supplied through the public Rust
    /// configuration structs.
    ///
    /// Deserialization rejects the same invalid values at the file/config
    /// boundary. Embedded callers that construct [`Config`] directly must call
    /// this method before passing it to measurement-only APIs;
    /// [`crate::evaluate_checks`] always calls it before inspecting or
    /// executing the supplied check catalog.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigValidationError::InvalidCheckSetting`] when a direct
    /// per-check numeric policy is outside its documented finite domain,
    /// [`ConfigValidationError::InvalidClipLoopCap`] when a clip selector
    /// contains a negative or non-finite per-clip loop cap,
    /// [`ConfigValidationError::ConflictingClipMovementOwner`] when one clip
    /// selector declares both `movement_owner_xz` and its `in_place` alias,
    /// [`ConfigValidationError::ConflictingRuntimeNodeSelectors`] when shared
    /// runtime-node selectors and the rest-world-scale compatibility alias are
    /// both declared,
    /// [`ConfigValidationError::InvalidSyncGroupTolerance`] when a same-time
    /// group has an invalid timing tolerance, or
    /// [`ConfigValidationError::InvalidTimeComplementSetting`] when an
    /// enabled time-complement policy has an invalid threshold.
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        if self.runtime_nodes.selectors.is_some()
            && self
                .checks
                .get("rest-world-scale")
                .is_some_and(|settings| settings.node_selectors.is_some())
        {
            return Err(ConfigValidationError::ConflictingRuntimeNodeSelectors);
        }
        for (check_id, settings) in &self.checks {
            for (field, valid) in [
                (
                    "max_ratio",
                    settings.max_ratio.is_none_or(is_nonnegative_finite),
                ),
                (
                    "min_stride_step_m",
                    settings.min_stride_step_m.is_none_or(is_nonnegative_finite),
                ),
                (
                    "max_position_delta_m",
                    settings
                        .max_position_delta_m
                        .is_none_or(is_nonnegative_finite),
                ),
                (
                    "max_rotation_delta_deg",
                    settings
                        .max_rotation_delta_deg
                        .is_none_or(is_nonnegative_finite),
                ),
                (
                    "max_velocity_delta_mps",
                    settings
                        .max_velocity_delta_mps
                        .is_none_or(is_nonnegative_finite),
                ),
                (
                    "max_angular_velocity_delta_degps",
                    settings
                        .max_angular_velocity_delta_degps
                        .is_none_or(is_nonnegative_finite),
                ),
                (
                    "min_rotation_deg",
                    settings.min_rotation_deg.is_none_or(is_nonnegative_finite),
                ),
                (
                    "max_mean_rest_delta_deg",
                    settings
                        .max_mean_rest_delta_deg
                        .is_none_or(is_nonnegative_finite),
                ),
                (
                    "contact_height_m",
                    settings.contact_height_m.is_none_or(is_nonnegative_finite),
                ),
                (
                    "max_slide_mps",
                    settings.max_slide_mps.is_none_or(is_nonnegative_finite),
                ),
                (
                    "expected_uniform_scale",
                    settings
                        .expected_uniform_scale
                        .is_none_or(is_positive_finite),
                ),
                (
                    "uniform_scale_tolerance",
                    settings
                        .uniform_scale_tolerance
                        .is_none_or(is_nonnegative_finite),
                ),
            ] {
                if !valid {
                    return Err(ConfigValidationError::InvalidCheckSetting {
                        check_id: check_id.clone(),
                        field,
                    });
                }
            }
        }
        for (selector, expectations) in &self.clips {
            if expectations.movement_owner_xz.is_some() && expectations.in_place.is_some() {
                return Err(ConfigValidationError::ConflictingClipMovementOwner {
                    selector: selector.clone(),
                });
            }
            for (field, value) in [
                (
                    "max_loop_position_delta_m",
                    expectations.max_loop_position_delta_m,
                ),
                (
                    "max_loop_rotation_delta_deg",
                    expectations.max_loop_rotation_delta_deg,
                ),
                (
                    "max_loop_velocity_delta_mps",
                    expectations.max_loop_velocity_delta_mps,
                ),
                (
                    "max_loop_angular_velocity_delta_degps",
                    expectations.max_loop_angular_velocity_delta_degps,
                ),
            ] {
                if value.is_some_and(|value| !is_nonnegative_finite(value)) {
                    return Err(ConfigValidationError::InvalidClipLoopCap {
                        selector: selector.clone(),
                        field,
                    });
                }
            }
        }
        for (group, sync) in &self.sync_groups {
            for (field, value) in [
                ("max_duration_delta_s", sync.max_duration_delta_s),
                ("max_fps_delta", sync.max_fps_delta),
            ] {
                if !is_nonnegative_finite(value) {
                    return Err(ConfigValidationError::InvalidSyncGroupTolerance {
                        group: group.clone(),
                        field,
                    });
                }
            }
            if let Some(settings) = &sync.time_complement {
                for (field, valid) in [
                    (
                        "min_reflected_time_advantage",
                        is_unit_interval(settings.min_reflected_time_advantage),
                    ),
                    (
                        "min_lr_amplitude_m",
                        is_nonnegative_finite(settings.min_lr_amplitude_m),
                    ),
                ] {
                    if !valid {
                        return Err(ConfigValidationError::InvalidTimeComplementSetting {
                            group: group.clone(),
                            field,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    /// Effective expectations for a clip: glob matches (in key order)
    /// overlaid, exact match last.
    ///
    /// Each selector entry's legacy `in_place` input is normalized into
    /// [`ClipExpectations::movement_owner_xz`] before the field overlay. The
    /// returned value therefore always has [`ClipExpectations::in_place`] set
    /// to `None`. Call [`Config::validate`] before this method so same-entry
    /// alias conflicts are rejected rather than resolved by precedence.
    pub fn expectations_for(&self, clip: &str) -> ClipExpectations {
        let mut out = ClipExpectations::default();
        for (pattern, exp) in &self.clips {
            if pattern != clip && glob_match(pattern, clip) {
                out = out.merged_with(exp);
            }
        }
        if let Some(exact) = self.clips.get(clip) {
            out = out.merged_with(exact);
        }
        out
    }

    /// Settings for a check id, or defaults when the id is not present.
    pub fn check_settings(&self, id: &str) -> CheckSettings {
        self.checks.get(id).cloned().unwrap_or_default()
    }

    /// The normalized runtime-node authority, if one is declared.
    ///
    /// The shared [`RuntimeNodesConfig::selectors`] field is used when it is
    /// present. Callers must first call [`Self::validate`]: simultaneous use
    /// of the legacy `checks.rest-world-scale.node_selectors` alias is rejected
    /// and has no precedence rule. An absent field or explicit empty list
    /// returns `None` and declares no policy.
    pub fn runtime_node_selectors(&self) -> Option<RuntimeNodeSelectors> {
        let selectors = self.runtime_nodes.selectors.as_ref().or_else(|| {
            self.checks
                .get("rest-world-scale")
                .and_then(|settings| settings.node_selectors.as_ref())
        })?;
        (!selectors.is_empty()).then(|| RuntimeNodeSelectors::new(selectors))
    }

    /// Effective stride floor for loop-seam metrics, in metres.
    pub fn loop_seam_min_stride_step_m(&self) -> f64 {
        self.check_settings("loop-seam")
            .min_stride_step_m
            .unwrap_or(MIN_STRIDE_STEP_M)
    }

    /// Resolve the exact loop-pose and seam-velocity tolerances for `clip`.
    ///
    /// Per-clip values win over their owning check settings, which in turn
    /// fall back to the immutable built-in check defaults. Call
    /// [`Self::validate`] before using configuration supplied by an untrusted
    /// decoder.
    pub fn loop_continuity_tolerances(&self, clip: &str) -> LoopContinuityTolerances {
        let expectations = self.expectations_for(clip);
        self.loop_continuity_tolerances_for(&expectations)
    }

    pub(crate) fn loop_continuity_tolerances_for(
        &self,
        expectations: &ClipExpectations,
    ) -> LoopContinuityTolerances {
        let (max_position_delta_m, max_rotation_delta_deg) =
            crate::checks::loop_closure::effective_caps(self, expectations);
        LoopContinuityTolerances {
            max_position_delta_m,
            max_rotation_delta_deg,
            max_velocity_delta_mps: crate::checks::loop_seam_vel::effective_cap(self, expectations),
            max_angular_velocity_delta_degps: crate::checks::loop_seam_rot::effective_cap(
                self,
                expectations,
            ),
        }
    }
}

/// Minimal linear-work `*`-wildcard matcher (no character classes; `*`
/// matches any run including empty).
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let pattern = pattern.as_bytes();
    let name = name.as_bytes();
    let Some(first_star) = pattern.iter().position(|&byte| byte == b'*') else {
        return pattern == name;
    };
    let last_star = pattern
        .iter()
        .rposition(|&byte| byte == b'*')
        .unwrap_or(first_star);

    let prefix = &pattern[..first_star];
    let suffix = &pattern[last_star + 1..];
    if !name.starts_with(prefix) || !name.ends_with(suffix) {
        return false;
    }

    let mut name_index = prefix.len();
    let search_end = name.len() - suffix.len();
    if name_index > search_end {
        return false;
    }

    if first_star < last_star {
        let mut failure = Vec::new();
        for literal in pattern[first_star + 1..last_star]
            .split(|&byte| byte == b'*')
            .filter(|literal| !literal.is_empty())
        {
            let Some(offset) =
                find_subslice_linear(&name[name_index..search_end], literal, &mut failure)
            else {
                return false;
            };
            name_index += offset + literal.len();
        }
    }

    true
}

fn find_subslice_linear(haystack: &[u8], needle: &[u8], failure: &mut Vec<usize>) -> Option<usize> {
    debug_assert!(!needle.is_empty());
    if needle.len() > haystack.len() {
        return None;
    }

    failure.clear();
    failure.resize(needle.len(), 0);
    let mut prefix_len = 0;
    for index in 1..needle.len() {
        while prefix_len > 0 && needle[index] != needle[prefix_len] {
            prefix_len = failure[prefix_len - 1];
        }
        if needle[index] == needle[prefix_len] {
            prefix_len += 1;
        }
        failure[index] = prefix_len;
    }

    let mut matched = 0;
    for (index, &byte) in haystack.iter().enumerate() {
        while matched > 0 && byte != needle[matched] {
            matched = failure[matched - 1];
        }
        if byte == needle[matched] {
            matched += 1;
            if matched == needle.len() {
                return Some(index + 1 - needle.len());
            }
        }
    }
    None
}
