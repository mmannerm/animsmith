//! Typed configuration: rig selection, per-check settings, per-clip
//! expectations, and typed clip groups. The TOML file (`animsmith.toml`) is
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
    /// `loop-seam`: ratio above which the seam is a pop (default 1.5).
    pub max_ratio: Option<f64>,
    /// `loop-seam`: stride floor in metres (default 0.02).
    pub min_stride_step_m: Option<f64>,
    /// `loop-closure`: maximum model-space position delta in metres
    /// (default 0.01).
    pub max_position_delta_m: Option<f64>,
    /// `loop-closure`: maximum model-space rotation delta in degrees
    /// (default 1.0).
    pub max_rotation_delta_deg: Option<f64>,
    /// `loop-seam-vel`: maximum incoming/outgoing model-space linear-velocity
    /// difference in metres per second (default 0.1).
    pub max_velocity_delta_mps: Option<f64>,
    /// `loop-seam-rot`: maximum incoming/outgoing model-space angular-velocity
    /// difference in degrees per second (default 5.0).
    pub max_angular_velocity_delta_degps: Option<f64>,
    /// `frozen-bone`: rotation floor in degrees (default 1.0).
    pub min_rotation_deg: Option<f64>,
    /// `bind-pose`: mean first-frame deviation cap in degrees
    /// (default 45).
    pub max_mean_rest_delta_deg: Option<f64>,
    /// `foot-slide`: contact height above the per-clip foot minimum
    /// (default 0.03 m).
    pub contact_height_m: Option<f64>,
    /// `foot-slide`: allowed stance-speed deviation (default 0.3 m/s).
    pub max_slide_mps: Option<f64>,
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
    /// Expected clip duration in seconds; consumed by the
    /// `duration-sanity` check. Its value must be finite and positive,
    /// and its tolerance must be finite and non-negative.
    pub duration_s: Option<Pinned>,
    /// Declared locomotion speed (m/s) carried by the clip's root
    /// motion.
    pub speed_mps: Option<Pinned>,
    /// The clip is authored in place (no net root travel); consumed by
    /// the `in-place` check (and exempts an in-place clip from
    /// `root-motion-speed`).
    pub in_place: Option<bool>,
    /// Authored frame rate; consumed by the `fps` check (keys must land
    /// on the `1/fps` grid).
    pub fps: Option<f64>,
    /// Bones that must carry keyframes and actually move
    /// (`missing-bones` presence + `frozen-bone` rotation floor).
    pub animates_bones: Option<Vec<String>>,
}

impl ClipExpectations {
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
            duration_s: other.duration_s.or(self.duration_s),
            speed_mps: other.speed_mps.or(self.speed_mps),
            in_place: other.in_place.or(self.in_place),
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
        && !is_valid_loop_cap(value)
    {
        return Err(serde::de::Error::custom(
            "must be a finite non-negative number",
        ));
    }
    Ok(value)
}

fn is_valid_loop_cap(value: f64) -> bool {
    value.is_finite() && value >= 0.0
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

/// Invalid values in a directly constructed [`Config`].
///
/// Numeric values are intentionally not retained in this error so it remains
/// equality-comparable even when the rejected input was `NaN`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigValidationError {
    /// A per-clip loop-continuity cap was negative or non-finite.
    #[error("clip selector {selector:?} field {field:?} must be a finite non-negative number")]
    InvalidClipLoopCap {
        /// Exact clip name or glob containing the invalid cap.
        selector: String,
        /// Stable public [`ClipExpectations`] field name.
        field: &'static str,
    },
}

/// The whole configuration. Field names match the `animsmith.toml`
/// sections.
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
    /// Keyed by clip name or glob (`*` wildcards). An exact-name entry
    /// overrides glob entries; among globs, later (lexicographically
    /// greater) keys win on conflict.
    #[serde(default)]
    pub clips: BTreeMap<String, ClipExpectations>,
    /// Named gait groups consumed by the `gait-group` check.
    #[serde(default)]
    pub gait_groups: BTreeMap<String, GaitGroup>,
}

impl Config {
    /// Validate values that can also be supplied through the public Rust
    /// configuration structs.
    ///
    /// Deserialization rejects the same invalid values at the file/config
    /// boundary. Embedded callers that construct [`Config`] directly may call
    /// this method for an earlier error; [`crate::evaluate_checks`] always
    /// calls it before inspecting or executing the supplied check catalog.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigValidationError::InvalidClipLoopCap`] when a clip
    /// selector contains a negative or non-finite per-clip loop cap.
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        for (selector, expectations) in &self.clips {
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
            ] {
                if value.is_some_and(|value| !is_valid_loop_cap(value)) {
                    return Err(ConfigValidationError::InvalidClipLoopCap {
                        selector: selector.clone(),
                        field,
                    });
                }
            }
        }
        Ok(())
    }

    /// Effective expectations for a clip: glob matches (in key order)
    /// overlaid, exact match last.
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

    /// Effective stride floor for loop-seam metrics, in metres.
    pub fn loop_seam_min_stride_step_m(&self) -> f64 {
        self.check_settings("loop-seam")
            .min_stride_step_m
            .unwrap_or(MIN_STRIDE_STEP_M)
    }
}

/// Minimal `*`-wildcard matcher (no character classes; `*` matches any
/// run including empty).
pub fn glob_match(pattern: &str, name: &str) -> bool {
    fn inner(p: &[u8], n: &[u8]) -> bool {
        match p.split_first() {
            None => n.is_empty(),
            Some((b'*', rest)) => (0..=n.len()).any(|skip| inner(rest, &n[skip..])),
            Some((c, rest)) => n
                .split_first()
                .is_some_and(|(nc, nrest)| nc == c && inner(rest, nrest)),
        }
    }
    inner(pattern.as_bytes(), name.as_bytes())
}
