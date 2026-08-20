use animsmith_core::config::{ClipExpectations, Config, ConfigValidationError, MovementOwner};
use animsmith_core::{
    CheckCtx, CheckSelection, Document, EvaluationError, MetricGrids, ResolvedRoles, all_checks,
    evaluate_checks,
};

fn parse_config(source: &str) -> Config {
    let config: Config = toml::from_str(source).expect("valid movement-owner config");
    config.validate().expect("valid direct config values");
    config
}

fn direct_config(entries: impl IntoIterator<Item = (String, ClipExpectations)>) -> Config {
    let mut config = Config::default();
    config.clips.extend(entries);
    config.validate().expect("valid direct config entries");
    config
}

#[test]
fn toml_accepts_both_owners_for_each_axis() {
    for (value, expected) in [
        ("gameplay", MovementOwner::Gameplay),
        ("animation", MovementOwner::Animation),
    ] {
        let config = parse_config(&format!(
            "[clips.motion]\n\
             movement_owner_xz = \"{value}\"\n\
             movement_owner_y = \"{value}\"\n\
             movement_owner_yaw = \"{value}\"\n"
        ));
        let effective = config.expectations_for("motion");
        assert_eq!(effective.movement_owner_xz, Some(expected));
        assert_eq!(effective.movement_owner_y, Some(expected));
        assert_eq!(effective.movement_owner_yaw, Some(expected));
        assert_eq!(effective.in_place, None);
    }
}

#[test]
fn legacy_boolean_alias_normalizes_to_horizontal_owner() {
    for (value, expected) in [
        (true, MovementOwner::Gameplay),
        (false, MovementOwner::Animation),
    ] {
        let config = parse_config(&format!("[clips.motion]\nin_place = {value}\n"));
        let effective = config.expectations_for("motion");
        assert_eq!(effective.movement_owner_xz, Some(expected));
        assert_eq!(effective.movement_owner_y, None);
        assert_eq!(effective.movement_owner_yaw, None);
        assert_eq!(effective.in_place, None);
    }
}

#[test]
fn exact_and_glob_layers_resolve_movement_fields_independently() {
    let config = parse_config(
        r#"
[clips."walk_*"]
in_place = true
movement_owner_y = "gameplay"
movement_owner_yaw = "animation"

[clips.walk_forward]
movement_owner_xz = "animation"
movement_owner_y = "animation"
movement_owner_yaw = "gameplay"
"#,
    );
    let walk = config.expectations_for("walk_forward");
    assert_eq!(walk.movement_owner_xz, Some(MovementOwner::Animation));
    assert_eq!(walk.movement_owner_y, Some(MovementOwner::Animation));
    assert_eq!(walk.movement_owner_yaw, Some(MovementOwner::Gameplay));
    assert_eq!(walk.in_place, None);

    let inherited = config.expectations_for("walk_left");
    assert_eq!(inherited.movement_owner_xz, Some(MovementOwner::Gameplay));
    assert_eq!(inherited.movement_owner_y, Some(MovementOwner::Gameplay));
    assert_eq!(inherited.movement_owner_yaw, Some(MovementOwner::Animation));
}

#[test]
fn canonical_glob_can_be_overlaid_by_exact_legacy_alias() {
    let config = parse_config(
        r#"
[clips."turn_*"]
movement_owner_xz = "animation"
movement_owner_yaw = "animation"

[clips.turn_left]
in_place = true
"#,
    );
    let turn = config.expectations_for("turn_left");
    assert_eq!(turn.movement_owner_xz, Some(MovementOwner::Gameplay));
    assert_eq!(turn.movement_owner_y, None);
    assert_eq!(turn.movement_owner_yaw, Some(MovementOwner::Animation));
    assert_eq!(turn.in_place, None);
}

#[test]
fn later_matching_glob_wins_only_for_its_declared_fields() {
    let config = parse_config(
        r#"
[clips."*"]
movement_owner_xz = "gameplay"
movement_owner_y = "animation"

[clips."walk_*"]
movement_owner_xz = "animation"
movement_owner_yaw = "gameplay"
"#,
    );
    let walk = config.expectations_for("walk_forward");
    assert_eq!(walk.movement_owner_xz, Some(MovementOwner::Animation));
    assert_eq!(walk.movement_owner_y, Some(MovementOwner::Animation));
    assert_eq!(walk.movement_owner_yaw, Some(MovementOwner::Gameplay));
}

#[test]
fn same_selector_alias_conflict_is_a_typed_error() {
    let config: Config = toml::from_str(
        r#"
[clips.walk]
movement_owner_xz = "gameplay"
in_place = true
"#,
    )
    .expect("both spellings deserialize before semantic validation");
    assert_eq!(
        config.validate(),
        Err(ConfigValidationError::ConflictingClipMovementOwner {
            selector: "walk".into(),
        })
    );

    let mut direct = Config::default();
    direct.clips.insert(
        "walk_*".into(),
        ClipExpectations {
            movement_owner_xz: Some(MovementOwner::Animation),
            in_place: Some(false),
            ..ClipExpectations::default()
        },
    );
    assert_eq!(
        direct.validate(),
        Err(ConfigValidationError::ConflictingClipMovementOwner {
            selector: "walk_*".into(),
        })
    );

    let doc = Document::default();
    let roles = ResolvedRoles::default();
    let grids = MetricGrids::new(&doc);
    let ctx = CheckCtx::new(&grids, &roles, &direct);
    assert_eq!(
        evaluate_checks(&ctx, &all_checks(), CheckSelection::All).unwrap_err(),
        EvaluationError::InvalidConfiguration(
            ConfigValidationError::ConflictingClipMovementOwner {
                selector: "walk_*".into(),
            }
        )
    );
}

#[test]
fn unknown_owner_values_and_fields_are_rejected_at_deserialization() {
    for value in ["engine", "controller", "GAMEPLAY", ""] {
        let unknown_value = toml::from_str::<Config>(&format!(
            r#"
[clips.walk]
movement_owner_xz = "{value}"
"#,
        ))
        .expect_err("unknown owner must fail");
        assert!(unknown_value.to_string().contains("unknown variant"));
    }

    let unknown_field = toml::from_str::<Config>(
        r#"
[clips.walk]
movement_owner_roll = "gameplay"
"#,
    )
    .expect_err("unknown movement field must fail");
    assert!(unknown_field.to_string().contains("unknown field"));
}

#[test]
fn clip_categories_keep_all_three_axes_independent() {
    let config = parse_config(
        r#"
[clips.turn_in_place]
movement_owner_xz = "gameplay"
movement_owner_y = "gameplay"
movement_owner_yaw = "animation"

[clips.jump]
movement_owner_xz = "gameplay"
movement_owner_y = "animation"
movement_owner_yaw = "gameplay"

[clips.run_forward]
movement_owner_xz = "animation"
movement_owner_y = "gameplay"
movement_owner_yaw = "animation"
"#,
    );

    let turn = config.expectations_for("turn_in_place");
    assert_eq!(turn.movement_owner_xz, Some(MovementOwner::Gameplay));
    assert_eq!(turn.movement_owner_y, Some(MovementOwner::Gameplay));
    assert_eq!(turn.movement_owner_yaw, Some(MovementOwner::Animation));

    let jump = config.expectations_for("jump");
    assert_eq!(jump.movement_owner_xz, Some(MovementOwner::Gameplay));
    assert_eq!(jump.movement_owner_y, Some(MovementOwner::Animation));
    assert_eq!(jump.movement_owner_yaw, Some(MovementOwner::Gameplay));

    let run = config.expectations_for("run_forward");
    assert_eq!(run.movement_owner_xz, Some(MovementOwner::Animation));
    assert_eq!(run.movement_owner_y, Some(MovementOwner::Gameplay));
    assert_eq!(run.movement_owner_yaw, Some(MovementOwner::Animation));

    let absent = config.expectations_for("idle");
    assert_eq!(absent.movement_owner_xz, None);
    assert_eq!(absent.movement_owner_y, None);
    assert_eq!(absent.movement_owner_yaw, None);
}

#[test]
fn direct_api_uses_the_same_canonical_owner_values() {
    for owner in [MovementOwner::Gameplay, MovementOwner::Animation] {
        let config = direct_config([(
            "motion".into(),
            ClipExpectations {
                movement_owner_xz: Some(owner),
                movement_owner_y: Some(owner),
                movement_owner_yaw: Some(owner),
                ..ClipExpectations::default()
            },
        )]);

        let effective = config.expectations_for("motion");
        assert_eq!(effective.normalized_movement_owner_xz(), Some(owner));
        assert_eq!(effective.movement_owner_y, Some(owner));
        assert_eq!(effective.movement_owner_yaw, Some(owner));
    }
}

#[test]
fn direct_api_preserves_alias_partial_and_layering_contracts() {
    for (in_place, expected) in [
        (true, MovementOwner::Gameplay),
        (false, MovementOwner::Animation),
    ] {
        let config = direct_config([(
            "motion".into(),
            ClipExpectations {
                in_place: Some(in_place),
                ..ClipExpectations::default()
            },
        )]);
        let effective = config.expectations_for("motion");
        assert_eq!(effective.movement_owner_xz, Some(expected));
        assert_eq!(effective.movement_owner_y, None);
        assert_eq!(effective.movement_owner_yaw, None);
        assert_eq!(effective.in_place, None);
    }

    let alias_glob = direct_config([
        (
            "walk_*".into(),
            ClipExpectations {
                in_place: Some(true),
                movement_owner_y: Some(MovementOwner::Animation),
                ..ClipExpectations::default()
            },
        ),
        (
            "walk_forward".into(),
            ClipExpectations {
                movement_owner_xz: Some(MovementOwner::Animation),
                movement_owner_yaw: Some(MovementOwner::Gameplay),
                ..ClipExpectations::default()
            },
        ),
    ]);
    let walk = alias_glob.expectations_for("walk_forward");
    assert_eq!(walk.movement_owner_xz, Some(MovementOwner::Animation));
    assert_eq!(walk.movement_owner_y, Some(MovementOwner::Animation));
    assert_eq!(walk.movement_owner_yaw, Some(MovementOwner::Gameplay));
    assert_eq!(walk.in_place, None);

    let canonical_glob = direct_config([
        (
            "turn_*".into(),
            ClipExpectations {
                movement_owner_xz: Some(MovementOwner::Animation),
                movement_owner_yaw: Some(MovementOwner::Animation),
                ..ClipExpectations::default()
            },
        ),
        (
            "turn_left".into(),
            ClipExpectations {
                in_place: Some(true),
                ..ClipExpectations::default()
            },
        ),
    ]);
    let turn = canonical_glob.expectations_for("turn_left");
    assert_eq!(turn.movement_owner_xz, Some(MovementOwner::Gameplay));
    assert_eq!(turn.movement_owner_y, None);
    assert_eq!(turn.movement_owner_yaw, Some(MovementOwner::Animation));
    assert_eq!(turn.in_place, None);

    let partial = direct_config([(
        "jump".into(),
        ClipExpectations {
            movement_owner_y: Some(MovementOwner::Animation),
            ..ClipExpectations::default()
        },
    )]);
    let jump = partial.expectations_for("jump");
    assert_eq!(jump.movement_owner_xz, None);
    assert_eq!(jump.movement_owner_y, Some(MovementOwner::Animation));
    assert_eq!(jump.movement_owner_yaw, None);
}

#[test]
fn undeclared_movement_ownership_is_not_inferred_from_clip_names() {
    let config = Config::default();
    for clip in ["run_RM", "jump_in_place", "turn_90", "ordinary"] {
        let effective = config.expectations_for(clip);
        assert_eq!(effective.movement_owner_xz, None, "clip {clip}");
        assert_eq!(effective.movement_owner_y, None, "clip {clip}");
        assert_eq!(effective.movement_owner_yaw, None, "clip {clip}");
        assert_eq!(effective.in_place, None, "clip {clip}");
    }
}
