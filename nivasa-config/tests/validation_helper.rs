use std::collections::BTreeMap;

use nivasa_config::{
    ConfigModule, ConfigValidationError, ConfigValidationIssue,
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
