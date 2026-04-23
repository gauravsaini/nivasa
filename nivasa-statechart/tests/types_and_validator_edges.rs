use nivasa_statechart::types::{Event, EventType};
use nivasa_statechart::validator::{validate, ValidationRule};
use nivasa_statechart::ScxmlDocument;
use serde_json::json;

#[test]
fn event_helpers_cover_external_data_and_descriptor_edges() {
    let event = Event::external("error.platform").with_data(json!({ "code": 500 }));

    assert_eq!(event.name, "error.platform");
    assert_eq!(event.event_type, EventType::External);
    assert_eq!(event.data, Some(json!({ "code": 500 })));
    assert!(event.matches_descriptor("error"));
    assert!(event.matches_descriptor("error.platform"));
    assert!(!event.matches_descriptor("error.platform.extra"));
}

#[test]
fn validator_reports_nondeterministic_and_invalid_event_edges() {
    let doc = ScxmlDocument::from_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<scxml xmlns="http://www.w3.org/2005/07/scxml" version="1.0" initial="idle">
  <state id="idle">
    <transition event="bad event" target="done"/>
    <transition event="bad event" target="done"/>
  </state>
  <final id="done"/>
</scxml>"#,
    )
    .expect("edge SCXML should parse");
    let result = validate(&doc);

    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.rule == ValidationRule::NonDeterministic));
    assert!(result
        .errors
        .iter()
        .any(|error| error.rule == ValidationRule::InvalidEventName));
}
