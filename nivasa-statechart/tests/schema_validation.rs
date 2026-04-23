use nivasa_statechart::{
    validate_scxml_schema, SchemaDiagnostic, SchemaDiagnostics, SchemaValidationError,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn statecharts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../statecharts")
}

fn temp_scxml_path(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("nivasa-{name}-{stamp}.scxml"))
}

#[test]
fn all_checked_in_statecharts_validate_against_the_w3c_scxml_xsd() {
    let mut files: Vec<_> = fs::read_dir(statecharts_dir())
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("scxml"))
        .collect();
    files.sort();

    assert!(
        !files.is_empty(),
        "expected at least one checked-in SCXML file"
    );

    for file in files {
        validate_scxml_schema(&file)
            .unwrap_or_else(|err| panic!("schema validation failed for {}: {err}", file.display()));
    }
}

#[test]
fn schema_invalid_scxml_returns_a_schema_error() {
    let path = temp_scxml_path("schema-invalid");
    fs::write(
        &path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<scxml xmlns="http://www.w3.org/2005/07/scxml" version="1.0" initial="a">
  <state id="a">
    <bogus/>
  </state>
</scxml>"#,
    )
    .unwrap();

    let result = validate_scxml_schema(&path);
    fs::remove_file(&path).ok();

    match result {
        Err(SchemaValidationError::Invalid {
            path: error_path,
            diagnostics,
        }) => {
            assert_eq!(error_path, path);
            assert!(
                !diagnostics.0.is_empty(),
                "expected at least one XSD diagnostic"
            );
            let rendered = format!(
                "{}",
                SchemaValidationError::Invalid {
                    path: error_path,
                    diagnostics: diagnostics.clone(),
                }
            );
            assert!(
                rendered.contains(path.to_string_lossy().as_ref()),
                "error display should mention file path: {rendered}"
            );
            assert!(
                rendered.contains("W3C SCXML XSD schema"),
                "error display should mention schema validation: {rendered}"
            );
        }
        other => panic!("expected XSD validation failure, got {other:?}"),
    }
}

#[test]
fn schema_diagnostics_display_location_and_multiline_collections() {
    let first = SchemaDiagnostic {
        level: "warning".to_string(),
        message: "first".to_string(),
        filename: Some("chart.scxml".to_string()),
        line: Some(7),
        column: Some(3),
        domain: 10,
        code: 20,
    };
    assert_eq!(
        first.to_string(),
        "chart.scxml:7:3: first (level=warning, domain=10, code=20)"
    );

    let second = SchemaDiagnostic {
        level: "none".to_string(),
        message: "second".to_string(),
        filename: None,
        line: None,
        column: Some(9),
        domain: 11,
        code: 21,
    };
    let diagnostics = SchemaDiagnostics(vec![first, second]);
    assert_eq!(
        diagnostics.to_string(),
        "chart.scxml:7:3: first (level=warning, domain=10, code=20)\n:9: second (level=none, domain=11, code=21)"
    );

    let load_error = SchemaValidationError::SchemaLoad {
        schema_path: PathBuf::from("schema.xsd"),
        diagnostics,
    };
    assert!(load_error
        .to_string()
        .contains("failed to load W3C SCXML XSD schema"));
}
