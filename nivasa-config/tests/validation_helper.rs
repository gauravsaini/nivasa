use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use nivasa_config::{
    ConfigBootstrapError, ConfigException, ConfigModule, ConfigOptions, ConfigSchema,
    ConfigService, ConfigValidationError, ConfigValidationIssue,
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

struct BootstrapSchema;

impl ConfigSchema for BootstrapSchema {
    fn required_keys() -> &'static [&'static str] {
        &[
            "NIVASA_PHASE6_TEST_HOST",
            "NIVASA_PHASE6_TEST_PORT",
            "NIVASA_PHASE6_TEST_API_KEY",
        ]
    }

    fn validate(loaded: &BTreeMap<String, String>) -> Vec<ConfigValidationIssue> {
        loaded
            .get("NIVASA_PHASE6_TEST_PORT")
            .and_then(|port| {
                port.parse::<u16>()
                    .err()
                    .map(|_| ConfigValidationIssue::InvalidValue {
                        key: "NIVASA_PHASE6_TEST_PORT".to_string(),
                        value: port.to_string(),
                        expected: "unsigned 16-bit integer".to_string(),
                    })
            })
            .into_iter()
            .collect()
    }
}

struct DefaultedSchema;

impl ConfigSchema for DefaultedSchema {
    fn required_keys() -> &'static [&'static str] {
        &["HOST", "PORT", "API_KEY"]
    }

    fn defaults() -> &'static [(&'static str, &'static str)] {
        &[("PORT", "3000"), ("SCHEME", "https")]
    }

    fn validate(loaded: &BTreeMap<String, String>) -> Vec<ConfigValidationIssue> {
        loaded
            .get("PORT")
            .and_then(|port| {
                port.parse::<u16>()
                    .err()
                    .map(|_| ConfigValidationIssue::InvalidValue {
                        key: "PORT".to_string(),
                        value: port.to_string(),
                        expected: "unsigned 16-bit integer".to_string(),
                    })
            })
            .into_iter()
            .collect()
    }
}

struct StrictlyValidatedSchema;

impl ConfigSchema for StrictlyValidatedSchema {
    fn required_keys() -> &'static [&'static str] {
        &["HOST", "PORT"]
    }

    fn validate(loaded: &BTreeMap<String, String>) -> Vec<ConfigValidationIssue> {
        let mut issues = Vec::new();

        if let Some(host) = loaded.get("HOST") {
            if !host.starts_with("127.") {
                issues.push(ConfigValidationIssue::InvalidValue {
                    key: "HOST".to_string(),
                    value: host.to_string(),
                    expected: "loopback address".to_string(),
                });
            }
        }

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

#[test]
fn config_schema_validation_applies_defaults_without_overriding_loaded_values() {
    let loaded = BTreeMap::from([
        ("HOST".to_string(), "127.0.0.1".to_string()),
        ("API_KEY".to_string(), "secret".to_string()),
    ]);

    let validated = ConfigModule::validate_schema::<DefaultedSchema>(&loaded)
        .expect("schema validation should succeed");

    assert_eq!(validated.get("HOST").map(String::as_str), Some("127.0.0.1"));
    assert_eq!(validated.get("API_KEY").map(String::as_str), Some("secret"));
    assert_eq!(validated.get("PORT").map(String::as_str), Some("3000"));
    assert_eq!(validated.get("SCHEME").map(String::as_str), Some("https"));
}

#[test]
fn config_service_get_helpers_cover_default_fallback_and_missing_keys() {
    let service = ConfigService::from_values(BTreeMap::from([
        ("PORT".to_string(), "3000".to_string()),
        ("BROKEN_PORT".to_string(), "abc".to_string()),
    ]));

    assert_eq!(service.get_raw("PORT"), Some("3000"));
    assert_eq!(service.get::<u16>("PORT"), Some(3000));
    assert_eq!(service.get::<u16>("BROKEN_PORT"), None);
    assert_eq!(service.get_or_default("BROKEN_PORT", 80), 80);
    assert_eq!(service.get_or_default("MISSING", 80), 80);

    assert_eq!(
        service.get_or_throw("MISSING"),
        Err(ConfigException::MissingKey {
            key: "MISSING".to_string(),
        })
    );
}

#[test]
fn load_env_parses_export_prefixes_quotes_and_invalid_lines() {
    let path = write_temp_env_file(
        "# comment line\n\
export NIVASA_CONFIG_TEST_EXPORTED=\"from export\"\n\
NIVASA_CONFIG_TEST_SINGLE='single quoted'\n\
NIVASA_CONFIG_TEST_SPACED =  spaced value  \n\
=ignored\n\
not-an-assignment\n",
    );
    let options = ConfigOptions::new().with_env_file_path(path.to_string_lossy().to_string());

    let loaded = ConfigModule::load_env(&options).expect("env file should load");

    assert_eq!(
        loaded
            .get("NIVASA_CONFIG_TEST_EXPORTED")
            .map(String::as_str),
        Some("from export")
    );
    assert_eq!(
        loaded.get("NIVASA_CONFIG_TEST_SINGLE").map(String::as_str),
        Some("single quoted")
    );
    assert_eq!(
        loaded.get("NIVASA_CONFIG_TEST_SPACED").map(String::as_str),
        Some("spaced value")
    );
    assert!(!loaded.contains_key("ignored"));
    assert!(!loaded.contains_key("not-an-assignment"));
}

#[test]
fn validate_schema_aggregates_custom_validation_issues_with_required_keys() {
    let loaded = BTreeMap::from([
        ("HOST".to_string(), "10.0.0.1".to_string()),
        ("PORT".to_string(), "abc".to_string()),
    ]);

    let error = ConfigModule::validate_schema::<StrictlyValidatedSchema>(&loaded)
        .expect_err("schema validation should fail");

    assert_eq!(
        error.issues(),
        &[
            ConfigValidationIssue::InvalidValue {
                key: "HOST".to_string(),
                value: "10.0.0.1".to_string(),
                expected: "loopback address".to_string(),
            },
            ConfigValidationIssue::InvalidValue {
                key: "PORT".to_string(),
                value: "abc".to_string(),
                expected: "unsigned 16-bit integer".to_string(),
            },
        ]
    );
}

fn write_temp_env_file(contents: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    path.push(format!("nivasa-config-{stamp}-{}.env", std::process::id()));
    fs::write(&path, contents).expect("env file should be writable");
    path
}

#[test]
fn config_root_with_schema_auto_validates_loaded_config_before_returning() {
    let env_file = write_temp_env_file(
        "NIVASA_PHASE6_TEST_HOST=127.0.0.1\n\
NIVASA_PHASE6_TEST_PORT=3000\n\
NIVASA_PHASE6_TEST_API_KEY=secret\n",
    );

    let module = ConfigModule::for_root_with_schema::<BootstrapSchema>(
        ConfigOptions::new().with_env_file_path(env_file.to_string_lossy().into_owned()),
    )
    .expect("schema validation should pass before returning the module");

    assert!(module.run_pre_bootstrap().is_ok());
}

#[test]
fn config_root_with_schema_rejects_missing_and_invalid_loaded_values() {
    let env_file = write_temp_env_file(
        "NIVASA_PHASE6_TEST_HOST=127.0.0.1\n\
NIVASA_PHASE6_TEST_PORT=abc\n",
    );

    let error = ConfigModule::for_root_with_schema::<BootstrapSchema>(
        ConfigOptions::new().with_env_file_path(env_file.to_string_lossy().into_owned()),
    )
    .expect_err("schema validation should fail fast");

    match error {
        ConfigBootstrapError::Validation { message } => {
            assert!(message.contains("missing required config key: NIVASA_PHASE6_TEST_API_KEY"));
            assert!(message.contains("invalid config value for NIVASA_PHASE6_TEST_PORT: abc"));
            assert!(message.contains("unsigned 16-bit integer"));
        }
        other => panic!("unexpected error: {other}"),
    }
}
