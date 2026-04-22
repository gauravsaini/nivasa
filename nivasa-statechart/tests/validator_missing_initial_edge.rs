use nivasa_statechart::parser::ScxmlDocument;
use nivasa_statechart::validator::{validate, ValidationRule};

#[test]
fn validator_reports_missing_initial_when_document_has_no_states() {
    let doc = ScxmlDocument::from_str(
        r#"<?xml version="1.0"?>
<scxml version="1.0" xmlns="http://www.w3.org/2005/07/scxml"/>"#,
    )
    .unwrap();

    let result = validate(&doc);

    assert!(!result.is_valid());
    assert!(result.errors.iter().any(|error| {
        error.rule == ValidationRule::MissingInitial
            && error.message == "No initial state specified and no top-level states found"
            && error.state_id.is_none()
    }));
}
