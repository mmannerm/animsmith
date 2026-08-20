use animsmith_core::config::{CheckSettings, Config, ConfigValidationError};

const NONNEGATIVE_CHECK_SETTINGS: [(&str, &str); 10] = [
    ("loop-seam", "max_ratio"),
    ("loop-seam", "min_stride_step_m"),
    ("loop-closure", "max_position_delta_m"),
    ("loop-closure", "max_rotation_delta_deg"),
    ("loop-seam-vel", "max_velocity_delta_mps"),
    ("loop-seam-rot", "max_angular_velocity_delta_degps"),
    ("frozen-bone", "min_rotation_deg"),
    ("bind-pose", "max_mean_rest_delta_deg"),
    ("foot-slide", "contact_height_m"),
    ("foot-slide", "max_slide_mps"),
];

fn settings_with(field: &str, value: f64) -> CheckSettings {
    let mut settings = CheckSettings::default();
    match field {
        "max_ratio" => settings.max_ratio = Some(value),
        "min_stride_step_m" => settings.min_stride_step_m = Some(value),
        "max_position_delta_m" => settings.max_position_delta_m = Some(value),
        "max_rotation_delta_deg" => settings.max_rotation_delta_deg = Some(value),
        "max_velocity_delta_mps" => settings.max_velocity_delta_mps = Some(value),
        "max_angular_velocity_delta_degps" => {
            settings.max_angular_velocity_delta_degps = Some(value);
        }
        "min_rotation_deg" => settings.min_rotation_deg = Some(value),
        "max_mean_rest_delta_deg" => settings.max_mean_rest_delta_deg = Some(value),
        "contact_height_m" => settings.contact_height_m = Some(value),
        "max_slide_mps" => settings.max_slide_mps = Some(value),
        _ => panic!("unknown check setting {field}"),
    }
    settings
}

fn settings_value(settings: &CheckSettings, field: &str) -> Option<f64> {
    match field {
        "max_ratio" => settings.max_ratio,
        "min_stride_step_m" => settings.min_stride_step_m,
        "max_position_delta_m" => settings.max_position_delta_m,
        "max_rotation_delta_deg" => settings.max_rotation_delta_deg,
        "max_velocity_delta_mps" => settings.max_velocity_delta_mps,
        "max_angular_velocity_delta_degps" => settings.max_angular_velocity_delta_degps,
        "min_rotation_deg" => settings.min_rotation_deg,
        "max_mean_rest_delta_deg" => settings.max_mean_rest_delta_deg,
        "contact_height_m" => settings.contact_height_m,
        "max_slide_mps" => settings.max_slide_mps,
        _ => panic!("unknown check setting {field}"),
    }
}

#[test]
fn deserialized_check_settings_reject_negative_and_non_finite_values() {
    for (check_id, field) in NONNEGATIVE_CHECK_SETTINGS {
        for value in ["-0.01", "nan", "inf", "-inf"] {
            let source = format!("[checks.{check_id}]\n{field} = {value}\n");
            assert!(
                toml::from_str::<Config>(&source).is_err(),
                "accepted invalid {check_id}.{field}={value}"
            );
        }

        let source = format!("[checks.{check_id}]\n{field} = 0.0\n");
        let config: Config = toml::from_str(&source)
            .unwrap_or_else(|error| panic!("rejected {check_id}.{field}=0.0: {error}"));
        assert_eq!(
            settings_value(&config.check_settings(check_id), field),
            Some(0.0),
            "did not preserve {check_id}.{field}=0.0"
        );
    }
}

#[test]
fn programmatic_check_settings_return_typed_validation_errors() {
    for (check_id, field) in NONNEGATIVE_CHECK_SETTINGS {
        for value in [-0.01, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut config = Config::default();
            config
                .checks
                .insert(check_id.into(), settings_with(field, value));

            assert_eq!(
                config.validate(),
                Err(ConfigValidationError::InvalidCheckSetting {
                    check_id: check_id.into(),
                    field,
                }),
                "{check_id}.{field}={value}"
            );
        }

        let mut config = Config::default();
        config
            .checks
            .insert(check_id.into(), settings_with(field, 0.0));
        config
            .validate()
            .unwrap_or_else(|error| panic!("rejected {check_id}.{field}=0.0: {error}"));
    }
}
