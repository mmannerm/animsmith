use animsmith_core::config::{
    CheckSettings, RuntimeNodeSelectorResolution, RuntimeNodesConfig, glob_match,
};
use animsmith_core::{Config, ConfigValidationError};

fn direct_config(selectors: Option<&[&str]>) -> Config {
    Config {
        runtime_nodes: RuntimeNodesConfig {
            selectors: selectors.map(|selectors| {
                selectors
                    .iter()
                    .map(|selector| (*selector).to_owned())
                    .collect()
            }),
        },
        ..Config::default()
    }
}

#[test]
fn direct_shared_runtime_node_policy_is_normalized_and_resolved_deterministically() {
    let config = direct_config(Some(&["socket", "hand_*", "socket", "missing"]));
    config.validate().expect("shared policy is valid");
    let selectors = config
        .runtime_node_selectors()
        .expect("non-empty policy is an authority");

    assert_eq!(selectors.selectors(), ["socket", "hand_*", "missing"]);
    assert_eq!(
        selectors.resolve([("socket", 1_u8), ("hand_l", 2), ("hand_r", 3)]),
        vec![
            RuntimeNodeSelectorResolution::ExactlyOne {
                selector: "socket".into(),
                node: 1,
            },
            RuntimeNodeSelectorResolution::Ambiguous {
                selector: "hand_*".into(),
                nodes: vec![2, 3],
            },
            RuntimeNodeSelectorResolution::NoMatch {
                selector: "missing".into(),
            },
        ]
    );
}

#[test]
fn absent_or_empty_shared_and_legacy_selector_fields_declare_no_policy() {
    for config in [
        Config::default(),
        direct_config(Some(&[])),
        Config {
            checks: [(
                "rest-world-scale".into(),
                CheckSettings {
                    node_selectors: Some(Vec::new()),
                    ..CheckSettings::default()
                },
            )]
            .into(),
            ..Config::default()
        },
    ] {
        config.validate().expect("empty policy is valid");
        assert!(config.runtime_node_selectors().is_none());
    }
}

#[test]
fn toml_shared_runtime_node_policy_preserves_first_selector_occurrence() {
    let config: Config = toml::from_str(
        r#"
            [runtime_nodes]
            selectors = ["socket", "hand_*", "socket"]
        "#,
    )
    .expect("shared TOML parses");
    config.validate().expect("shared TOML is valid");
    assert_eq!(
        config
            .runtime_node_selectors()
            .expect("shared policy is an authority")
            .selectors(),
        ["socket", "hand_*"]
    );

    let empty: Config =
        toml::from_str("[runtime_nodes]\nselectors = []").expect("empty shared TOML parses");
    empty.validate().expect("empty shared TOML is valid");
    assert!(empty.runtime_node_selectors().is_none());
}

#[test]
fn legacy_selector_alias_is_used_only_when_shared_selector_field_is_absent() {
    let config: Config = toml::from_str(
        r#"
            [checks.rest-world-scale]
            node_selectors = ["socket", "socket"]
        "#,
    )
    .expect("legacy TOML parses");
    config.validate().expect("legacy alias is valid");
    assert_eq!(
        config
            .runtime_node_selectors()
            .expect("legacy policy is an authority")
            .selectors(),
        ["socket"]
    );
}

#[test]
fn toml_rejects_simultaneous_shared_and_legacy_selector_forms() {
    for source in [
        r#"
            [runtime_nodes]
            selectors = ["socket"]

            [checks.rest-world-scale]
            node_selectors = ["socket"]
        "#,
        r#"
            [runtime_nodes]
            selectors = []

            [checks.rest-world-scale]
            node_selectors = []
        "#,
    ] {
        let config: Config = toml::from_str(source).expect("both forms parse before validation");
        assert_eq!(
            config.validate(),
            Err(ConfigValidationError::ConflictingRuntimeNodeSelectors)
        );
    }
}

#[test]
fn direct_config_rejects_simultaneous_shared_and_legacy_selector_forms() {
    let mut config = direct_config(Some(&["socket"]));
    config.checks.insert(
        "rest-world-scale".into(),
        CheckSettings {
            node_selectors: Some(vec!["socket".into()]),
            ..CheckSettings::default()
        },
    );

    assert_eq!(
        config.validate(),
        Err(ConfigValidationError::ConflictingRuntimeNodeSelectors)
    );
}

#[test]
fn toml_runtime_nodes_rejects_unknown_fields() {
    let error = toml::from_str::<Config>(
        r#"
            [runtime_nodes]
            selector = ["socket"]
        "#,
    )
    .expect_err("runtime-node config must be closed");
    assert!(error.to_string().contains("selector"));
}

#[test]
fn glob_match_preserves_semantics_with_linear_hostile_input_work() {
    for (pattern, name, expected) in [
        ("", "", true),
        ("", "socket", false),
        ("*", "", true),
        ("*", "socket", true),
        ("socket", "socket", true),
        ("socket", "Socket", false),
        ("hand_*", "hand_左", true),
        ("*左", "右左", true),
        ("*左", "右", false),
        ("*ab", "aab", true),
        ("*aba", "aaba", true),
        ("a*a", "a", false),
        ("*aba*aba*", "ababa", false),
        ("*aba*aba*", "abaaba", true),
        ("a**b***c", "abc", true),
        ("a**b***c", "a---b+++c", true),
        ("a**b***c", "a---b+++d", false),
    ] {
        assert_eq!(
            glob_match(pattern, name),
            expected,
            "{pattern:?} vs {name:?}"
        );
    }

    let deep_stars = "*".repeat(32_768);
    assert!(glob_match(&deep_stars, ""));

    let hostile_literal = "a".repeat(32_768);
    let hostile_pattern = format!("*{hostile_literal}b*");
    let hostile_name = "a".repeat(32_769);
    assert!(!glob_match(&hostile_pattern, &hostile_name));
}
