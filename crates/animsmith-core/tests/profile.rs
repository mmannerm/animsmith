//! Rig-profile resolution and auto-detection.

use animsmith_core::Config;
use animsmith_core::model::{Bone, Skeleton, Transform};
use animsmith_core::profile::{
    ResolutionOutcome, ResolvedRoles, RigProfile, Role, RoleResolutionPolicy, detect_profile,
    detect_profile_detailed, resolve_configured_roles, resolve_named_detailed,
};

fn skeleton_of(names: &[&str]) -> Skeleton {
    Skeleton {
        bones: names
            .iter()
            .enumerate()
            .map(|(i, name)| Bone {
                name: (*name).into(),
                parent: if i == 0 { None } else { Some(0) },
                rest: Transform::IDENTITY,
                inverse_bind: None,
            })
            .collect(),
    }
}

#[test]
fn detects_humanoid_prefixed() {
    let skel = skeleton_of(&[
        "root",
        "humanoid_ Pelvis",
        "humanoid_ L Foot",
        "humanoid_ R Foot",
        "humanoid_ L Toe0",
        "humanoid_ R Toe0",
    ]);
    let roles = detect_profile(&skel).expect("profile detected");
    assert_eq!(roles.profile, "humanoid");
    assert_eq!(roles.get(Role::Hips), Some(1));
    assert_eq!(roles.get(Role::LeftToe), Some(4));
}

#[test]
fn builtins_accept_unique_ascii_case_only_bindings_without_displacing_exact_matches() {
    let skel = skeleton_of(&[
        "root",
        "humanoid_ Pelvis",
        "Humanoid_ L Foot",
        "Humanoid_ R Foot",
        "Humanoid_ L Toe0",
        "Humanoid_ R Toe0",
    ]);
    let named = resolve_named_detailed(&skel, "humanoid");
    let automatic = detect_profile_detailed(&skel);
    for roles in [&named, &automatic] {
        assert_eq!(roles.profile, "humanoid");
        assert_eq!(roles.outcome(), ResolutionOutcome::Coverage);
        assert_eq!(roles.get(Role::Hips), Some(1));
        assert_eq!(roles.policy(Role::Hips), Some(RoleResolutionPolicy::Exact));
        assert_eq!(
            roles.policy(Role::LeftFoot),
            Some(RoleResolutionPolicy::AsciiCaseInsensitive)
        );
    }
}

#[test]
fn builtin_exact_match_wins_when_a_folded_candidate_for_the_same_binding_also_exists() {
    let resolved = resolve_named_detailed(
        &skeleton_of(&["humanoid_ L Foot", "Humanoid_ L Foot"]),
        "humanoid",
    );

    assert_eq!(resolved.outcome(), ResolutionOutcome::Coverage);
    assert_eq!(resolved.get(Role::LeftFoot), Some(0));
    assert_eq!(
        resolved.policy(Role::LeftFoot),
        Some(RoleResolutionPolicy::Exact)
    );
}

#[test]
fn custom_profiles_remain_exact_only_while_builtins_allow_case_only_fallback() {
    use animsmith_core::profile::NameMatcher::Exact;

    let skeleton = skeleton_of(&["hips"]);
    let custom = RigProfile {
        name: "custom",
        bindings: vec![(Role::Hips, Exact("Hips"))],
    }
    .resolve(&skeleton);
    assert_eq!(custom.outcome(), ResolutionOutcome::Coverage);
    assert!(custom.is_empty());

    let builtin = resolve_named_detailed(
        &skeleton_of(&["Humanoid_ Pelvis", "Humanoid_ L Foot"]),
        "humanoid",
    );
    assert_eq!(
        builtin.policy(Role::Hips),
        Some(RoleResolutionPolicy::AsciiCaseInsensitive)
    );
}

#[test]
fn folded_duplicates_are_an_observable_ambiguity_not_a_declaration_order_pick() {
    let skel = skeleton_of(&[
        "root",
        "Humanoid_ Pelvis",
        "HUMANOID_ PELVIS",
        "Humanoid_ L Foot",
        "Humanoid_ R Foot",
    ]);
    let named = resolve_named_detailed(&skel, "humanoid");
    assert_eq!(named.outcome(), ResolutionOutcome::AmbiguousFoldedMatch);
    assert!(named.is_empty());
    let configured = resolve_configured_roles(
        &skel,
        &serde_json::from_value::<Config>(serde_json::json!({
            "rig": { "profile": "humanoid" }
        }))
        .unwrap()
        .rig,
    );
    assert_eq!(
        configured.outcome(),
        ResolutionOutcome::AmbiguousFoldedMatch
    );
    assert!(configured.is_empty());
}

#[test]
fn role_collisions_and_auto_ties_are_typed_and_fail_closed() {
    use animsmith_core::profile::NameMatcher::Exact;

    let collision = RigProfile {
        name: "synthetic",
        bindings: vec![
            (Role::Hips, Exact("shared")),
            (Role::Spine, Exact("shared")),
        ],
    }
    .resolve(&skeleton_of(&["shared"]));
    assert_eq!(collision.outcome(), ResolutionOutcome::RoleCollision);
    assert!(collision.is_empty());

    let tie = detect_profile_detailed(&skeleton_of(&[
        "mixamorig:Hips",
        "mixamorig:LeftFoot",
        "pelvis",
        "foot_l",
    ]));
    assert_eq!(tie.outcome(), ResolutionOutcome::AmbiguousProfile);
    assert!(tie.is_empty());
    assert!(
        detect_profile(&skeleton_of(&[
            "mixamorig:Hips",
            "mixamorig:LeftFoot",
            "pelvis",
            "foot_l",
        ]))
        .is_none()
    );
}

#[test]
fn auto_does_not_select_a_lower_score_profile_around_folded_ambiguity() {
    let roles = detect_profile_detailed(&skeleton_of(&[
        "root",
        "pelvis",
        "foot_l",
        "Humanoid_ Pelvis",
        "HUMANOID_ PELVIS",
    ]));

    assert_eq!(roles.outcome(), ResolutionOutcome::AmbiguousFoldedMatch);
    assert!(roles.is_empty());
}

#[test]
fn detects_mixamo_with_namespace() {
    let skel = skeleton_of(&[
        "Armature",
        "mixamorig:Hips",
        "mixamorig:LeftFoot",
        "mixamorig:RightFoot",
    ]);
    let roles = detect_profile(&skel).expect("profile detected");
    assert_eq!(roles.profile, "mixamo");
    assert_eq!(roles.get(Role::Hips), Some(1));
}

#[test]
fn detects_ue_mannequin() {
    let skel = skeleton_of(&["root", "pelvis", "foot_l", "foot_r", "ball_l", "ball_r"]);
    let roles = detect_profile(&skel).expect("profile detected");
    assert_eq!(roles.profile, "ue-mannequin");
    assert_eq!(roles.get(Role::Root), Some(0));
    assert_eq!(roles.get(Role::RightToe), Some(5));
}

#[test]
fn unknown_rig_detects_nothing() {
    let skel = skeleton_of(&["a", "b", "c"]);
    assert!(detect_profile(&skel).is_none());
}

#[test]
fn explicit_names_report_coverage_for_absent_bindings_and_last_resolved_pair_wins() {
    let skel = skeleton_of(&["first", "second"]);
    let roles = ResolvedRoles::from_names(
        &skel,
        [
            (Role::Hips, "absent".to_string()),
            (Role::Root, "first".to_string()),
            (Role::Root, "second".to_string()),
            (Role::Root, "also-absent".to_string()),
        ],
    );

    assert_eq!(roles.get(Role::Root), Some(1));
    assert_eq!(roles.get(Role::Hips), None);
    assert_eq!(roles.outcome(), ResolutionOutcome::Coverage);
}

#[test]
fn configured_resolution_applies_inline_roles_over_the_named_profile() {
    let skel = skeleton_of(&["root", "pelvis", "foot_l", "foot_r", "custom_foot"]);
    let config: Config = serde_json::from_value(serde_json::json!({
        "rig": {
            "profile": "ue-mannequin",
            "roles": { "left_foot": "custom_foot" }
        }
    }))
    .unwrap();

    let roles = resolve_configured_roles(&skel, &config.rig);
    assert_eq!(roles.profile, "ue-mannequin+custom");
    assert_eq!(roles.get(Role::Root), Some(0));
    assert_eq!(roles.get(Role::LeftFoot), Some(4));
    assert_eq!(roles.get(Role::RightFoot), Some(3));
    assert_eq!(
        roles.policy(Role::LeftFoot),
        Some(RoleResolutionPolicy::Explicit)
    );
    assert_eq!(
        roles.policy(Role::RightFoot),
        Some(RoleResolutionPolicy::Exact)
    );
}

#[test]
fn explicit_roles_are_exact_and_collision_free() {
    let skel = skeleton_of(&["Humanoid_ Pelvis", "custom"]);
    let case_sensitive: Config = serde_json::from_value(serde_json::json!({
        "rig": { "roles": { "hips": "humanoid_ Pelvis" } }
    }))
    .unwrap();
    let unresolved = resolve_configured_roles(&skel, &case_sensitive.rig);
    assert!(unresolved.is_empty());
    assert_eq!(unresolved.outcome(), ResolutionOutcome::Coverage);

    let collision_skel = skeleton_of(&["root", "pelvis", "foot_l", "foot_r"]);
    let collision: Config = serde_json::from_value(serde_json::json!({
        "rig": {
            "profile": "ue-mannequin",
            "roles": { "left_foot": "foot_r" }
        }
    }))
    .unwrap();
    let refused = resolve_configured_roles(&collision_skel, &collision.rig);
    assert_eq!(refused.outcome(), ResolutionOutcome::RoleCollision);
    assert!(refused.is_empty());
}

#[test]
fn unrelated_explicit_role_does_not_hide_a_folded_profile_ambiguity() {
    let skel = skeleton_of(&[
        "root",
        "Humanoid_ Pelvis",
        "HUMANOID_ PELVIS",
        "custom_foot",
    ]);
    let config: Config = serde_json::from_value(serde_json::json!({
        "rig": {
            "profile": "humanoid",
            "roles": { "left_foot": "custom_foot" }
        }
    }))
    .unwrap();

    let roles = resolve_configured_roles(&skel, &config.rig);
    assert_eq!(roles.outcome(), ResolutionOutcome::AmbiguousFoldedMatch);
    assert!(roles.is_empty());
}

#[test]
fn explicit_override_recovers_only_the_ambiguous_role_and_can_complete_profile_coverage() {
    let skel = skeleton_of(&[
        "root",
        "Humanoid_ Pelvis",
        "HUMANOID_ PELVIS",
        "humanoid_ Spine",
        "humanoid_ Head",
        "humanoid_ L Foot",
        "humanoid_ R Foot",
        "humanoid_ L Toe0",
        "humanoid_ R Toe0",
        "humanoid_ L Hand",
        "humanoid_ R Hand",
    ]);
    let config: Config = serde_json::from_value(serde_json::json!({
        "rig": {
            "profile": "humanoid",
            "roles": { "hips": "Humanoid_ Pelvis" }
        }
    }))
    .unwrap();

    let roles = resolve_configured_roles(&skel, &config.rig);
    assert_eq!(roles.outcome(), ResolutionOutcome::Resolved);
    assert_eq!(roles.get(Role::Hips), Some(1));
    assert_eq!(
        roles.policy(Role::Hips),
        Some(RoleResolutionPolicy::Explicit)
    );
    assert_eq!(roles.policy(Role::Spine), Some(RoleResolutionPolicy::Exact));
}

#[test]
fn explicit_bindings_can_complete_the_missing_named_profile_roles() {
    let skel = skeleton_of(&[
        "root",
        "pelvis",
        "configured_spine",
        "configured_head",
        "configured_left_foot",
        "configured_right_foot",
        "configured_left_toe",
        "configured_right_toe",
        "configured_left_hand",
        "configured_right_hand",
    ]);
    let config: Config = serde_json::from_value(serde_json::json!({
        "rig": {
            "profile": "ue-mannequin",
            "roles": {
                "spine": "configured_spine",
                "head": "configured_head",
                "left_foot": "configured_left_foot",
                "right_foot": "configured_right_foot",
                "left_toe": "configured_left_toe",
                "right_toe": "configured_right_toe",
                "left_hand": "configured_left_hand",
                "right_hand": "configured_right_hand"
            }
        }
    }))
    .unwrap();

    let roles = resolve_configured_roles(&skel, &config.rig);
    assert_eq!(roles.outcome(), ResolutionOutcome::Resolved);
    assert_eq!(
        roles.policy(Role::Spine),
        Some(RoleResolutionPolicy::Explicit)
    );
}

#[test]
fn auto_profile_scores_non_overridden_bindings_with_explicit_roles() {
    let skel = skeleton_of(&["configured_hips", "mixamorig:LeftFoot"]);
    let config: Config = serde_json::from_value(serde_json::json!({
        "rig": {
            "profile": "auto",
            "roles": { "hips": "configured_hips" }
        }
    }))
    .unwrap();

    let roles = resolve_configured_roles(&skel, &config.rig);
    assert_eq!(roles.profile, "mixamo+custom");
    assert_eq!(roles.outcome(), ResolutionOutcome::Coverage);
    assert_eq!(
        roles.policy(Role::Hips),
        Some(RoleResolutionPolicy::Explicit)
    );
    assert_eq!(
        roles.policy(Role::LeftFoot),
        Some(RoleResolutionPolicy::Exact)
    );
}

#[test]
fn auto_with_only_explicit_matches_keeps_the_custom_map_without_a_profile_tie() {
    let skel = skeleton_of(&["custom_hips", "custom_left", "custom_right"]);
    let config: Config = serde_json::from_value(serde_json::json!({
        "rig": {
            "profile": "auto",
            "roles": {
                "hips": "custom_hips",
                "left_foot": "custom_left",
                "right_foot": "custom_right"
            }
        }
    }))
    .unwrap();

    let roles = resolve_configured_roles(&skel, &config.rig);
    assert_eq!(roles.profile, "custom");
    assert_eq!(roles.outcome(), ResolutionOutcome::Resolved);
    assert_eq!(roles.get(Role::Hips), Some(0));
    assert_eq!(roles.get(Role::LeftFoot), Some(1));
    assert_eq!(roles.get(Role::RightFoot), Some(2));
}

#[test]
fn named_profile_without_an_unoverridden_match_remains_coverage_incomplete() {
    let skel = skeleton_of(&["custom_hips"]);
    let config: Config = serde_json::from_value(serde_json::json!({
        "rig": {
            "profile": "ue-mannequin",
            "roles": { "hips": "custom_hips" }
        }
    }))
    .unwrap();

    let roles = resolve_configured_roles(&skel, &config.rig);
    assert_eq!(roles.profile, "custom");
    assert_eq!(roles.outcome(), ResolutionOutcome::Coverage);
    assert_eq!(roles.get(Role::Hips), Some(0));
}

#[test]
fn configured_resolution_labels_unresolved_and_inline_only_rigs() {
    let skel = skeleton_of(&["pelvis_custom"]);
    let unknown = resolve_configured_roles(&skel, &Config::default().rig);
    assert_eq!(unknown.profile, "unknown");
    assert!(unknown.is_empty());

    let config: Config = serde_json::from_value(serde_json::json!({
        "rig": { "roles": { "hips": "pelvis_custom" } }
    }))
    .unwrap();
    let custom = resolve_configured_roles(&skel, &config.rig);
    assert_eq!(custom.profile, "custom");
    assert_eq!(custom.get(Role::Hips), Some(0));

    let invalid_inline: Config = serde_json::from_value(serde_json::json!({
        "rig": { "roles": { "hips": "absent" } }
    }))
    .unwrap();
    let unresolved = resolve_configured_roles(&skel, &invalid_inline.rig);
    assert_eq!(unresolved.profile, "unknown");
    assert!(unresolved.is_empty());

    let named_without_matches: Config = serde_json::from_value(serde_json::json!({
        "rig": { "profile": "ue-mannequin" }
    }))
    .unwrap();
    let unresolved = resolve_configured_roles(&skel, &named_without_matches.rig);
    assert_eq!(unresolved.profile, "unknown");
    assert!(unresolved.is_empty());

    let named_skel = skeleton_of(&["root", "pelvis", "foot_l", "foot_r"]);
    let invalid_override: Config = serde_json::from_value(serde_json::json!({
        "rig": {
            "profile": "ue-mannequin",
            "roles": { "left_foot": "absent" }
        }
    }))
    .unwrap();
    let named = resolve_configured_roles(&named_skel, &invalid_override.rig);
    assert_eq!(named.profile, "ue-mannequin");
    assert_eq!(named.get(Role::LeftFoot), None);
    assert_eq!(named.outcome(), ResolutionOutcome::Coverage);
}
