use nivasa_statechart::{HistoryType, ScxmlDocument, TransitionType};

#[test]
fn parser_applies_default_scxml_and_transition_values() {
    let scxml = r#"<?xml version="1.0" encoding="UTF-8"?>
<scxml xmlns="http://www.w3.org/2005/07/scxml" initial="start">
  <state id="start">
    <transition event="go" cond="ready"/>
    <history/>
    <invoke autoforward="false"/>
  </state>
</scxml>"#;

    let doc = ScxmlDocument::from_str(scxml).unwrap();

    assert_eq!(doc.metadata.version, "1.0");
    assert_eq!(doc.metadata.initial.as_deref(), Some("start"));

    let start = &doc.states["start"];
    assert_eq!(start.transitions.len(), 1);
    assert_eq!(start.transitions[0].event.as_deref(), Some("go"));
    assert_eq!(start.transitions[0].cond.as_deref(), Some("ready"));
    assert!(start.transitions[0].target.is_empty());
    assert_eq!(
        start.transitions[0].transition_type,
        TransitionType::External
    );

    assert_eq!(start.invoke.len(), 1);
    assert!(!start.invoke[0].autoforward);

    assert_eq!(doc.history_states.len(), 1);
    assert_eq!(doc.history_states[0].id, "");
    assert_eq!(doc.history_states[0].history_type, HistoryType::Shallow);
    assert_eq!(doc.history_states[0].parent, "start");
}
