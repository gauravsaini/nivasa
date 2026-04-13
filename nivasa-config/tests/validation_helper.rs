use std::collections::BTreeMap;

use nivasa_config::{
    ConfigModule, ConfigSchema, ConfigValidationError, ConfigValidationIssue,
};

#[test]
fn validate_required_keys_accepts_owned_required_keys() {
    let loaded = BTreeMap::from([
        ("HOST".to_string(), "127.0.0.1".to_string()),
        ("PORT".to_string(), "3000".to_string()),
    ]);
    let required_keys = vec![
        " HOST ".to_string(),
        "PORT".to_string(),
        "".to_string(),
        "PORT".to_string(),
    ];

    let result = ConfigModule::validate_required_keys(&loaded, required_keys);

    assert_eq!(result, Ok(()));
}

#[test]
fn config_validation_error_reports_empty_state_stably() {
    let error = ConfigValidationError::new(Vec::new());

    assert!(error.is_empty());
    assert!(error.issues().is_empty());
    assert_eq!(error.to_string(), "config validation failed");
}

#[test]
fn config_validation_issue_display_is_human_readable() {
    let issue = ConfigValidationIssue::MissingRequiredKey {
        key: "API_KEY".to_string(),
    };

    assert_eq!(issue.to_string(), "missing required config key: API_KEY");
}

#[test]
fn config_validation_issue_display_covers_invalid_values() {
    let issue = ConfigValidationIssue::InvalidValue {
        key: "PORT".to_string(),
        value: "abc".to_string(),
        expected: "unsigned integer".to_string(),
    };

    assert_eq!(
        issue.to_string(),
        "invalid config value for PORT: abc (unsigned integer)"
    );
}

struct StrictConfigSchema;

impl ConfigSchema for StrictConfigSchema {
    fn required_keys() -> &'static [&'static str] {
        &["HOST", "PORT", "API_KEY"]
    }

    fn defaults() -> &'static [(&'static str, &'static str)] {
        &[("SCHEME", "http")]
    }

    fn validate(loaded: &BTreeMap<String, String>) -> Vec<ConfigValidationIssue> {
        let mut issues = Vec::new();

        if let Some(port) = loaded.get("PORT") {
            if port.parse::<u16>().is_err() {
                issues.push(ConfigValidationIssue::InvalidValue {
                    key: "PORT".to_string(),
                    value: port.to_string(),
                    expected: "unsigned 16-bit integer".to_string(),
                });
            }
        }

        issues
    }
}

#[test]
fn config_schema_validation_reports_missing_and_invalid_values_together() {
    let loaded = BTreeMap::from([
        ("HOST".to_string(), "127.0.0.1".to_string()),
        ("PORT".to_string(), "abc".to_string()),
    ]);

    let error = ConfigModule::validate_schema::<StrictConfigSchema>(&loaded)
        .expect_err("schema validation should fail");

    assert_eq!(
        error.issues(),
        &[
            ConfigValidationIssue::MissingRequiredKey {
                key: "API_KEY".to_string(),
            },
            ConfigValidationIssue::InvalidValue {
                key: "PORT".to_string(),
                value: "abc".to_string(),
                expected: "unsigned 16-bit integer".to_string(),
            },
        ]
    );
    assert_eq!(
        error.to_string(),
        "missing required config key: API_KEY; invalid config value for PORT: abc (unsigned 16-bit integer)"
    );
}
