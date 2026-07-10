//! Rig-profile resolution and auto-detection.

use animsmith_core::model::{Bone, Skeleton, Transform};
use animsmith_core::profile::{ResolvedRoles, Role, detect_profile};

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
            (Role::Root, "also-absent".to_string()),
            (Role::Root, "second".to_string()),
        ],
    );

    assert_eq!(roles.get(Role::Root), Some(1));
    assert_eq!(roles.get(Role::Hips), None);
}
