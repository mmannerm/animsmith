use std::process::Command;

fn animsmith() -> Command {
    Command::new(env!("CARGO_BIN_EXE_animsmith"))
}

#[test]
fn exact_json_only_command_is_present_without_defaults_or_output_override() {
    let missing = tempfile::tempdir().unwrap();
    let manifest = missing.path().join("collection.toml");
    let parameterization = missing.path().join("foot-cycle.toml");
    let result = animsmith()
        .args([
            "collection",
            "transform-foot-cycle",
            manifest.to_str().unwrap(),
            "--parameterization",
            parameterization.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("foot-cycle source preparation failed (control)")
    );

    for (extra, diagnostic) in [
        (vec!["--output", "other"], "unexpected argument '--output'"),
        (vec!["--format", "text"], "invalid value 'text'"),
    ] {
        let mut command = animsmith();
        command.args([
            "collection",
            "transform-foot-cycle",
            manifest.to_str().unwrap(),
            "--parameterization",
            parameterization.to_str().unwrap(),
        ]);
        command.args(extra);
        let result = command.output().unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(
            String::from_utf8(result.stderr)
                .unwrap()
                .contains(diagnostic)
        );
    }

    let omitted_format = animsmith()
        .args([
            "collection",
            "transform-foot-cycle",
            manifest.to_str().unwrap(),
            "--parameterization",
            parameterization.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(omitted_format.status.code(), Some(2));
    assert!(omitted_format.stdout.is_empty());
    assert!(
        String::from_utf8(omitted_format.stderr)
            .unwrap()
            .contains("required arguments were not provided:\n  --format <FORMAT>")
    );
}

#[test]
fn collection_transform_refuses_global_document_config_before_loading() {
    let result = animsmith()
        .args([
            "--config",
            "config.toml",
            "collection",
            "transform-foot-cycle",
            "collection.toml",
            "--parameterization",
            "foot-cycle.toml",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("--config is not accepted by collection commands")
    );
}
