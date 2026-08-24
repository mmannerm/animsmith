//! Rig profiles: checks never reference bone names, they reference
//! *roles*. A profile maps roles to name matchers; built-ins cover the
//! common rigs and auto-detection scores every built-in by resolved-role
//! coverage. A check whose required roles do not resolve reports a typed
//! coverage gap — never a false failure.

use crate::config::RigConfig;
use crate::model::{BoneId, Skeleton};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

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
    /// Stable snake-case role name used in config and result messages.
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

/// Stable explanation of how one resolved role matched its delivered bone
/// name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RoleResolutionPolicy {
    /// A built-in binding matched exactly (including its established
    /// namespace-stripped form).
    Exact,
    /// A built-in binding had no exact candidate and matched one unique
    /// ASCII-case-insensitive candidate.
    AsciiCaseInsensitive,
    /// An exact user-supplied `[rig.roles]` binding supplied the name.
    Explicit,
}

impl RoleResolutionPolicy {
    /// Stable output-contract spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::AsciiCaseInsensitive => "ascii-case-insensitive",
            Self::Explicit => "explicit",
        }
    }
}

/// Typed overall result of resolving a configured rig.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ResolutionOutcome {
    /// Every binding considered by the selected profile resolved.
    Resolved,
    /// A usable, injective role map was produced but one or more bindings did
    /// not have a candidate.
    Coverage,
    /// One exact binding had more than one candidate.
    AmbiguousExactMatch,
    /// One case-insensitive binding had more than one candidate.
    AmbiguousFoldedMatch,
    /// Two roles would have selected the same bone.
    RoleCollision,
    /// More than one built-in profile had the same best automatic score.
    AmbiguousProfile,
}

impl ResolutionOutcome {
    /// Stable output-contract spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::Coverage => "coverage",
            Self::AmbiguousExactMatch => "ambiguous_exact_match",
            Self::AmbiguousFoldedMatch => "ambiguous_folded_match",
            Self::RoleCollision => "role_collision",
            Self::AmbiguousProfile => "ambiguous_profile",
        }
    }

    const fn is_ambiguous(self) -> bool {
        !matches!(self, Self::Resolved | Self::Coverage)
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
    fn exact_matches(&self, bone_name: &str) -> bool {
        let NameMatcher::Exact(wanted) = self;
        bone_name == *wanted
            || bone_name
                .rsplit_once(':')
                .is_some_and(|(_, stripped)| stripped == *wanted)
    }

    fn ascii_case_insensitive_matches(&self, bone_name: &str) -> bool {
        let NameMatcher::Exact(wanted) = self;
        bone_name.eq_ignore_ascii_case(wanted)
            || bone_name
                .rsplit_once(':')
                .is_some_and(|(_, stripped)| stripped.eq_ignore_ascii_case(wanted))
    }
}

/// A named set of role-to-bone-name matchers.
#[derive(Debug, Clone)]
pub struct RigProfile {
    /// Profile name used in configuration and result messages.
    pub name: &'static str,
    /// Role matchers tried against a skeleton.
    pub bindings: Vec<(Role, NameMatcher)>,
}

/// Role → bone resolution for one skeleton.
#[derive(Debug, Clone)]
pub struct ResolvedRoles {
    /// Name of the profile that produced this resolution ("custom" for
    /// inline role maps).
    pub profile: String,
    map: BTreeMap<Role, ResolvedBone>,
    outcome: ResolutionOutcome,
}

#[derive(Debug, Clone)]
struct ResolvedBone {
    id: BoneId,
    name: String,
    policy: RoleResolutionPolicy,
}

impl ResolvedRoles {
    /// Bone id for a role, when resolved.
    pub fn get(&self, role: Role) -> Option<BoneId> {
        self.map.get(&role).map(|bone| bone.id)
    }

    /// Resolution policy for a role, when it resolved.
    pub fn policy(&self, role: Role) -> Option<RoleResolutionPolicy> {
        self.map.get(&role).map(|bone| bone.policy)
    }

    /// Typed result of the overall profile/configuration resolution.
    pub const fn outcome(&self) -> ResolutionOutcome {
        self.outcome
    }

    /// Bone id and captured name for internal boundaries that must reject a
    /// role map reused with a different skeleton.
    pub(crate) fn get_with_name(&self, role: Role) -> Option<(BoneId, &str)> {
        self.map
            .get(&role)
            .map(|bone| (bone.id, bone.name.as_str()))
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
        self.map.iter().map(|(&role, bone)| (role, bone.id))
    }

    pub(crate) fn iter_with_details(
        &self,
    ) -> impl Iterator<Item = (Role, BoneId, &str, RoleResolutionPolicy)> + '_ {
        self.map
            .iter()
            .map(|(&role, bone)| (role, bone.id, bone.name.as_str(), bone.policy))
    }

    /// Build from explicit role → bone-name pairs (for example a config
    /// inline map). Pairs whose bone name is absent are not bound, but make
    /// the result coverage-incomplete; when a role appears more than once,
    /// the last resolved pair wins. A final map that would bind two roles to
    /// one bone is refused as a typed collision.
    pub fn from_names(
        skeleton: &Skeleton,
        names: impl IntoIterator<Item = (Role, String)>,
    ) -> Self {
        let mut map = BTreeMap::new();
        let mut coverage_gap = false;
        for (role, name) in names {
            if let Some(id) = skeleton.bones.iter().position(|b| b.name == name) {
                map.insert(
                    role,
                    ResolvedBone {
                        id,
                        name: skeleton.bones[id].name.clone(),
                        policy: RoleResolutionPolicy::Explicit,
                    },
                );
            } else {
                coverage_gap = true;
            }
        }
        let outcome = injective_outcome(&map).unwrap_or(if coverage_gap || map.is_empty() {
            ResolutionOutcome::Coverage
        } else {
            ResolutionOutcome::Resolved
        });
        Self {
            profile: "custom".into(),
            map: if outcome.is_ambiguous() {
                BTreeMap::new()
            } else {
                map
            },
            outcome,
        }
    }
}

impl Default for ResolvedRoles {
    fn default() -> Self {
        unresolved("unknown", ResolutionOutcome::Coverage)
    }
}

impl RigProfile {
    /// Resolve this profile against `skeleton` using established exact matching
    /// semantics. Built-in alias fallback is deliberately unavailable through
    /// this public custom-profile API.
    pub fn resolve(&self, skeleton: &Skeleton) -> ResolvedRoles {
        self.resolve_with_options(skeleton, &BTreeSet::new(), false)
    }

    fn resolve_builtin_excluding(
        &self,
        skeleton: &Skeleton,
        excluded_roles: &BTreeSet<Role>,
    ) -> ResolvedRoles {
        self.resolve_with_options(skeleton, excluded_roles, true)
    }

    fn resolve_with_options(
        &self,
        skeleton: &Skeleton,
        excluded_roles: &BTreeSet<Role>,
        allow_ascii_case_insensitive_fallback: bool,
    ) -> ResolvedRoles {
        let mut map = BTreeMap::new();
        let mut coverage_gap = false;
        for (role, matcher) in &self.bindings {
            if excluded_roles.contains(role) {
                continue;
            }
            let exact: Vec<_> = skeleton
                .bones
                .iter()
                .enumerate()
                .filter_map(|(id, bone)| matcher.exact_matches(&bone.name).then_some(id))
                .collect();
            let (id, policy) = match exact.as_slice() {
                [id] => (*id, RoleResolutionPolicy::Exact),
                [] if allow_ascii_case_insensitive_fallback => {
                    let folded: Vec<_> = skeleton
                        .bones
                        .iter()
                        .enumerate()
                        .filter_map(|(id, bone)| {
                            matcher
                                .ascii_case_insensitive_matches(&bone.name)
                                .then_some(id)
                        })
                        .collect();
                    match folded.as_slice() {
                        [id] => (*id, RoleResolutionPolicy::AsciiCaseInsensitive),
                        [] => {
                            coverage_gap = true;
                            continue;
                        }
                        _ => return unresolved(self.name, ResolutionOutcome::AmbiguousFoldedMatch),
                    }
                }
                [] => {
                    coverage_gap = true;
                    continue;
                }
                _ => return unresolved(self.name, ResolutionOutcome::AmbiguousExactMatch),
            };
            map.insert(
                *role,
                ResolvedBone {
                    id,
                    name: skeleton.bones[id].name.clone(),
                    policy,
                },
            );
        }
        if injective_outcome(&map).is_some() {
            return unresolved(self.name, ResolutionOutcome::RoleCollision);
        }
        ResolvedRoles {
            profile: self.name.into(),
            map,
            outcome: if coverage_gap {
                ResolutionOutcome::Coverage
            } else {
                ResolutionOutcome::Resolved
            },
        }
    }
}

fn unresolved(profile: &str, outcome: ResolutionOutcome) -> ResolvedRoles {
    ResolvedRoles {
        profile: profile.into(),
        map: BTreeMap::new(),
        outcome,
    }
}

fn injective_outcome(map: &BTreeMap<Role, ResolvedBone>) -> Option<ResolutionOutcome> {
    let mut ids = BTreeSet::new();
    map.values()
        .any(|bone| !ids.insert(bone.id))
        .then_some(ResolutionOutcome::RoleCollision)
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

/// Auto-detect with a typed result. A profile must resolve at least two roles;
/// equal best candidates are refused rather than chosen by declaration order.
pub fn detect_profile_detailed(skeleton: &Skeleton) -> ResolvedRoles {
    detect_profile_excluding(skeleton, &BTreeSet::new(), 0)
}

fn detect_profile_excluding(
    skeleton: &Skeleton,
    excluded_roles: &BTreeSet<Role>,
    score_offset: usize,
) -> ResolvedRoles {
    let resolved: Vec<_> = builtin_profiles()
        .iter()
        .map(|profile| profile.resolve_builtin_excluding(skeleton, excluded_roles))
        .collect();
    let ambiguities: Vec<_> = resolved
        .iter()
        .filter_map(|roles| roles.outcome.is_ambiguous().then_some(roles.outcome))
        .collect();
    if ambiguities.len() == 1 {
        return unresolved("unknown", ambiguities[0]);
    }
    if ambiguities.len() > 1 {
        return unresolved("unknown", ResolutionOutcome::AmbiguousProfile);
    }
    let candidates: Vec<_> = resolved
        .iter()
        .filter(|roles| {
            !roles.outcome.is_ambiguous() && !roles.is_empty() && roles.len() + score_offset >= 2
        })
        .cloned()
        .collect();
    let Some(best_score) = candidates.iter().map(ResolvedRoles::len).max() else {
        return ResolvedRoles::default();
    };
    let mut best = candidates
        .into_iter()
        .filter(|roles| roles.len() == best_score);
    let selected = best.next().expect("a best score has a candidate");
    if best.next().is_some() {
        unresolved("unknown", ResolutionOutcome::AmbiguousProfile)
    } else {
        selected
    }
}

/// Auto-detect a built-in profile, preserving the historical optional API.
/// Use [`detect_profile_detailed`] when a caller needs the typed outcome.
pub fn detect_profile(skeleton: &Skeleton) -> Option<ResolvedRoles> {
    let resolved = detect_profile_detailed(skeleton);
    (!resolved.is_empty() && !resolved.outcome.is_ambiguous()).then_some(resolved)
}

/// Resolve a profile by name, or auto-detect for `"auto"`, retaining a typed
/// ambiguity or coverage result for callers that publish it.
pub fn resolve_named_detailed(skeleton: &Skeleton, profile: &str) -> ResolvedRoles {
    if profile == "auto" {
        return detect_profile_detailed(skeleton);
    }
    builtin_profiles()
        .iter()
        .find(|candidate| candidate.name == profile)
        .map(|candidate| candidate.resolve_builtin_excluding(skeleton, &BTreeSet::new()))
        .unwrap_or_default()
}

/// Resolve a profile by name, or auto-detect for `"auto"`.
pub fn resolve_named(skeleton: &Skeleton, profile: &str) -> Option<ResolvedRoles> {
    if profile != "auto"
        && !builtin_profiles()
            .iter()
            .any(|candidate| candidate.name == profile)
    {
        return None;
    }
    let resolved = resolve_named_detailed(skeleton, profile);
    (!resolved.outcome.is_ambiguous()).then_some(resolved)
}

/// Resolve a configured rig profile and apply inline role overrides.
///
/// Inline role bindings win over bindings from the named or auto-detected
/// profile. They remain exact, while built-in bindings use only the documented
/// ASCII case-tolerant fallback. A collision is refused instead of selecting a
/// role by configuration or profile declaration order.
pub fn resolve_configured_roles(skeleton: &Skeleton, rig: &RigConfig) -> ResolvedRoles {
    let mut explicit = BTreeMap::new();
    let mut explicit_coverage_gap = false;
    let mut inline_contributed = false;
    for (&role, name) in &rig.roles {
        if let Some(id) = skeleton.bones.iter().position(|bone| bone.name == *name) {
            inline_contributed = true;
            explicit.insert(
                role,
                ResolvedBone {
                    id,
                    name: skeleton.bones[id].name.clone(),
                    policy: RoleResolutionPolicy::Explicit,
                },
            );
        } else {
            explicit_coverage_gap = true;
        }
    }
    if injective_outcome(&explicit).is_some() {
        return unresolved("unknown", ResolutionOutcome::RoleCollision);
    }
    let overridden_roles: BTreeSet<_> = rig.roles.keys().copied().collect();
    let base = if rig.profile == "auto" {
        detect_profile_excluding(skeleton, &overridden_roles, explicit.len())
    } else {
        builtin_profiles()
            .iter()
            .find(|profile| profile.name == rig.profile)
            .map(|profile| profile.resolve_builtin_excluding(skeleton, &overridden_roles))
            .unwrap_or_default()
    };
    if base.outcome.is_ambiguous() {
        return unresolved("unknown", base.outcome);
    }
    let base_contributed = !base.is_empty();
    let mut map = base.map;
    map.extend(explicit);
    if injective_outcome(&map).is_some() {
        return unresolved("unknown", ResolutionOutcome::RoleCollision);
    }
    let profile = match (base_contributed, inline_contributed) {
        (false, false) => "unknown".into(),
        (false, true) => "custom".into(),
        (true, false) => base.profile,
        (true, true) => format!("{}+custom", base.profile),
    };
    let outcome = if explicit_coverage_gap
        || (base.outcome == ResolutionOutcome::Coverage
            && (base_contributed || rig.profile != "auto"))
        || map.is_empty()
    {
        ResolutionOutcome::Coverage
    } else {
        ResolutionOutcome::Resolved
    };
    ResolvedRoles {
        profile,
        map,
        outcome,
    }
}
