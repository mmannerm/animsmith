//! Rig-profile resolution and auto-detection.

use animsmith_core::model::{Bone, Skeleton, Transform};
use animsmith_core::profile::{ResolvedRoles, Role, detect_profile};
use animsmith_core::{Config, resolve_configured_roles};

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
fn explicit_names_ignore_absent_bones_and_last_resolved_pair_wins() {
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
    assert_eq!(named.get(Role::LeftFoot), Some(2));
}
