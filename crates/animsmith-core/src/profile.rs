//! Rig profiles: checks never reference bone names, they reference
//! *roles*. A profile maps roles to name matchers; built-ins cover the
//! common rigs and auto-detection scores every built-in by resolved-role
//! coverage. A check whose required roles don't resolve is skipped with
//! a note — never a false failure.

use crate::config::RigConfig;
use crate::model::{BoneId, Skeleton};
use serde::Deserialize;
use std::collections::BTreeMap;

/// Semantic bone roles used by checks and measurements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Role {
    /// Scene or locomotion root.
    Root,
    /// Pelvis/hips control, used as the primary body reference.
    Hips,
    /// Spine control.
    Spine,
    /// Head control.
    Head,
    /// Left foot control.
    LeftFoot,
    /// Right foot control.
    RightFoot,
    /// Left toe control.
    LeftToe,
    /// Right toe control.
    RightToe,
    /// Left hand control.
    LeftHand,
    /// Right hand control.
    RightHand,
}

impl Role {
    /// Stable snake-case role name used in config and diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Root => "root",
            Role::Hips => "hips",
            Role::Spine => "spine",
            Role::Head => "head",
            Role::LeftFoot => "left_foot",
            Role::RightFoot => "right_foot",
            Role::LeftToe => "left_toe",
            Role::RightToe => "right_toe",
            Role::LeftHand => "left_hand",
            Role::RightHand => "right_hand",
        }
    }
}

/// How a role's bone is found by name. Matching also tries a
/// namespace-stripped variant of each bone name (`"ns:Hips"` → `"Hips"`).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum NameMatcher {
    /// Exact bone-name match, with namespace-stripped fallback.
    Exact(&'static str),
}

impl NameMatcher {
    fn matches(&self, bone_name: &str) -> bool {
        let NameMatcher::Exact(wanted) = self;
        if bone_name == *wanted {
            return true;
        }
        // Namespace-stripped fallback: "mixamorig:Hips" ~ "Hips".
        bone_name
            .rsplit_once(':')
            .is_some_and(|(_, stripped)| stripped == *wanted)
    }
}

/// A named set of role-to-bone-name matchers.
#[derive(Debug, Clone)]
pub struct RigProfile {
    /// Profile name used in configuration and diagnostics.
    pub name: &'static str,
    /// Role matchers tried against a skeleton.
    pub bindings: Vec<(Role, NameMatcher)>,
}

/// Role → bone resolution for one skeleton.
#[derive(Debug, Clone, Default)]
pub struct ResolvedRoles {
    /// Name of the profile that produced this resolution ("custom" for
    /// inline role maps).
    pub profile: String,
    map: BTreeMap<Role, BoneId>,
}

impl ResolvedRoles {
    /// Bone id for a role, when resolved.
    pub fn get(&self, role: Role) -> Option<BoneId> {
        self.map.get(&role).copied()
    }

    /// Number of resolved roles.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether no roles resolved.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Iterate resolved `(role, bone_id)` pairs in role order.
    pub fn iter(&self) -> impl Iterator<Item = (Role, BoneId)> + '_ {
        self.map.iter().map(|(&r, &b)| (r, b))
    }

    /// Build from explicit role → bone-name pairs (for example a config
    /// inline map). Pairs whose bone name is absent are ignored; when a role
    /// appears more than once, the last resolved pair wins.
    pub fn from_names(
        skeleton: &Skeleton,
        names: impl IntoIterator<Item = (Role, String)>,
    ) -> Self {
        let mut map = BTreeMap::new();
        for (role, name) in names {
            if let Some(id) = skeleton.bones.iter().position(|b| b.name == name) {
                map.insert(role, id);
            }
        }
        Self {
            profile: "custom".into(),
            map,
        }
    }
}

impl RigProfile {
    /// Resolve this profile against `skeleton` by matching bone names.
    pub fn resolve(&self, skeleton: &Skeleton) -> ResolvedRoles {
        let mut map = BTreeMap::new();
        for (role, matcher) in &self.bindings {
            if let Some(id) = skeleton.bones.iter().position(|b| matcher.matches(&b.name)) {
                map.insert(*role, id);
            }
        }
        ResolvedRoles {
            profile: self.name.into(),
            map,
        }
    }
}

/// The built-in profiles.
pub fn builtin_profiles() -> Vec<RigProfile> {
    use NameMatcher::Exact;
    use Role::*;
    vec![
        RigProfile {
            name: "mixamo",
            bindings: vec![
                (Hips, Exact("mixamorig:Hips")),
                (Spine, Exact("mixamorig:Spine")),
                (Head, Exact("mixamorig:Head")),
                (LeftFoot, Exact("mixamorig:LeftFoot")),
                (RightFoot, Exact("mixamorig:RightFoot")),
                (LeftToe, Exact("mixamorig:LeftToeBase")),
                (RightToe, Exact("mixamorig:RightToeBase")),
                (LeftHand, Exact("mixamorig:LeftHand")),
                (RightHand, Exact("mixamorig:RightHand")),
            ],
        },
        RigProfile {
            name: "ue-mannequin",
            bindings: vec![
                (Root, Exact("root")),
                (Hips, Exact("pelvis")),
                (Spine, Exact("spine_01")),
                (Head, Exact("head")),
                (LeftFoot, Exact("foot_l")),
                (RightFoot, Exact("foot_r")),
                (LeftToe, Exact("ball_l")),
                (RightToe, Exact("ball_r")),
                (LeftHand, Exact("hand_l")),
                (RightHand, Exact("hand_r")),
            ],
        },
        RigProfile {
            name: "humanoid",
            bindings: vec![
                (Root, Exact("root")),
                (Hips, Exact("humanoid_ Pelvis")),
                (Spine, Exact("humanoid_ Spine")),
                (Head, Exact("humanoid_ Head")),
                (LeftFoot, Exact("humanoid_ L Foot")),
                (RightFoot, Exact("humanoid_ R Foot")),
                (LeftToe, Exact("humanoid_ L Toe0")),
                (RightToe, Exact("humanoid_ R Toe0")),
                (LeftHand, Exact("humanoid_ L Hand")),
                (RightHand, Exact("humanoid_ R Hand")),
            ],
        },
    ]
}

/// Auto-detect: score every built-in by resolved-role coverage; the
/// best profile wins if it resolves at least two roles. Ties keep the
/// earlier (declaration-order) profile.
pub fn detect_profile(skeleton: &Skeleton) -> Option<ResolvedRoles> {
    builtin_profiles()
        .iter()
        .map(|p| p.resolve(skeleton))
        .filter(|r| r.len() >= 2)
        .max_by_key(ResolvedRoles::len)
}

/// Resolve a profile by name, or auto-detect for `"auto"`.
pub fn resolve_named(skeleton: &Skeleton, profile: &str) -> Option<ResolvedRoles> {
    if profile == "auto" {
        return detect_profile(skeleton);
    }
    builtin_profiles()
        .iter()
        .find(|p| p.name == profile)
        .map(|p| p.resolve(skeleton))
}

/// Resolve a configured rig profile and apply inline role overrides.
///
/// Inline role bindings win over bindings from the named or auto-detected
/// profile. Names absent from `skeleton` are ignored. The returned profile is
/// `"unknown"` when neither a profile nor inline binding resolves, `"custom"`
/// for inline-only resolution, or `<profile>+custom` when both contribute.
pub fn resolve_configured_roles(skeleton: &Skeleton, rig: &RigConfig) -> ResolvedRoles {
    let base = resolve_named(skeleton, &rig.profile).unwrap_or_default();
    if rig.roles.is_empty() {
        let mut roles = base;
        if roles.profile.is_empty() {
            roles.profile = "unknown".into();
        }
        return roles;
    }

    let mut pairs: Vec<_> = base
        .iter()
        .map(|(role, bone)| (role, skeleton.bones[bone].name.clone()))
        .collect();
    pairs.extend(rig.roles.iter().map(|(role, name)| (*role, name.clone())));

    let mut resolved = ResolvedRoles::from_names(skeleton, pairs);
    resolved.profile = if base.profile.is_empty() {
        "custom".into()
    } else {
        format!("{}+custom", base.profile)
    };
    resolved
}
