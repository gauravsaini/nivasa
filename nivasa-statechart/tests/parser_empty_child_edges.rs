use nivasa_statechart::parser::{ParseError, ScxmlDocument};

#[test]
fn self_closing_child_state_does_not_capture_parent_transition_context() {
    let scxml = r#"<?xml version="1.0" encoding="UTF-8"?>
<scxml xmlns="http://www.w3.org/2005/07/scxml" initial="outer">
  <state id="outer">
    <state id="child"/>
    <transition event="after_child" target="child"/>
  </state>
</scxml>"#;

    let doc = ScxmlDocument::from_str(scxml).unwrap();

    assert_eq!(doc.states["outer"].transitions.len(), 1);
    assert_eq!(
        doc.states["outer"].transitions[0].event.as_deref(),
        Some("after_child")
    );
    assert!(doc.states["child"].transitions.is_empty());
}

#[test]
fn self_closing_child_parallel_does_not_capture_parent_transition_context() {
    let scxml = r#"<?xml version="1.0" encoding="UTF-8"?>
<scxml xmlns="http://www.w3.org/2005/07/scxml" initial="outer">
  <state id="outer">
    <parallel id="regions"/>
    <transition event="after_parallel" target="regions"/>
  </state>
</scxml>"#;

    let doc = ScxmlDocument::from_str(scxml).unwrap();

    assert_eq!(doc.states["outer"].transitions.len(), 1);
    assert_eq!(
        doc.states["outer"].transitions[0].event.as_deref(),
        Some("after_parallel")
    );
    assert!(doc.states["regions"].transitions.is_empty());
}

#[test]
fn parser_reports_io_and_xml_errors_from_file_entrypoint() {
    let missing = std::env::temp_dir().join("nivasa-statechart-missing-planck.scxml");
    let err = ScxmlDocument::from_file(&missing).unwrap_err();
    assert!(matches!(err, ParseError::Io(_)));

    let malformed = std::env::temp_dir().join("nivasa-statechart-malformed-planck.scxml");
    std::fs::write(&malformed, "<scxml><state id=\"broken\"></scxml>").unwrap();

    let err = ScxmlDocument::from_file(&malformed).unwrap_err();
    std::fs::remove_file(&malformed).ok();

    assert!(matches!(err, ParseError::Xml(_)));
}

#[test]
fn state_ids_and_event_names_cover_eventless_and_duplicate_transitions() {
    let scxml = r#"<?xml version="1.0" encoding="UTF-8"?>
<scxml xmlns="http://www.w3.org/2005/07/scxml" initial="alpha">
  <state id="alpha">
    <transition event="repeat" target="beta"/>
    <transition event="repeat" target="gamma"/>
    <transition target="gamma"/>
  </state>
  <state id="beta"/>
  <final id="gamma"/>
</scxml>"#;

    let doc = ScxmlDocument::from_str(scxml).unwrap();

    let mut state_ids = doc.state_ids().into_iter().cloned().collect::<Vec<_>>();
    state_ids.sort();
    assert_eq!(state_ids, vec!["alpha", "beta", "gamma"]);

    assert_eq!(doc.event_names(), vec!["repeat".to_string()]);
}
