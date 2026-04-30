//! SCXML document parser.
//!
//! Parses `.scxml` files (W3C State Chart XML) into an in-memory
//! [`ScxmlDocument`] representation.
//!
//! This parser only builds the document model. It does not perform W3C SCXML
//! schema validation; use [`crate::schema::validate_scxml_schema`] for that.
//!
//! # Example
//!
//! ```rust
//! use nivasa_statechart::ScxmlDocument;
//!
//! let scxml = r#"<?xml version="1.0" encoding="UTF-8"?>
//! <scxml xmlns="http://www.w3.org/2005/07/scxml" version="1.0" initial="idle">
//!   <state id="idle"/>
//! </scxml>"#;
//!
//! let doc = ScxmlDocument::from_str(scxml).unwrap();
//! assert_eq!(doc.metadata.initial.as_deref(), Some("idle"));
//! assert!(doc.has_state("idle"));
//! ```

use crate::types::*;
use quick_xml::events::{BytesStart, Event as XmlEvent};
use quick_xml::Reader;
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

/// Errors that can occur during SCXML parsing.
///
/// # Example
///
/// ```rust
/// use nivasa_statechart::parser::{ParseError, ScxmlDocument};
///
/// let err = ScxmlDocument::from_str("<state id=\"idle\"/>").unwrap_err();
/// assert!(matches!(err, ParseError::Invalid(message) if message.contains("Missing <scxml> root element")));
/// ```
#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("XML parsing error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Missing required attribute '{attribute}' on <{element}> at position {position}")]
    MissingAttribute {
        element: String,
        attribute: String,
        position: usize,
    },
    #[error("Invalid SCXML: {0}")]
    Invalid(String),
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("Attribute error: {0}")]
    AttrError(#[from] quick_xml::events::attributes::AttrError),
}

/// A parsed SCXML document.
///
/// Use [`ScxmlDocument::from_str`] for embedded SCXML content or
/// [`ScxmlDocument::from_file`] for files on disk.
///
/// # Example
///
/// ```rust
/// use nivasa_statechart::ScxmlDocument;
///
/// let scxml = r#"<?xml version="1.0" encoding="UTF-8"?>
/// <scxml xmlns="http://www.w3.org/2005/07/scxml" version="1.0" initial="idle">
///   <state id="idle"/>
/// </scxml>"#;
///
/// let doc = ScxmlDocument::from_str(scxml).unwrap();
/// assert_eq!(doc.top_level_states, vec!["idle".to_string()]);
/// ```
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ScxmlDocument {
    /// Metadata from the `<scxml>` root element.
    pub metadata: ScxmlMetadata,
    /// All states indexed by their ID.
    pub states: HashMap<StateId, State>,
    /// Top-level state IDs (direct children of `<scxml>`).
    pub top_level_states: Vec<StateId>,
    /// History pseudo-states.
    pub history_states: Vec<HistoryState>,
    /// The raw XML source (for hash computation).
    pub raw_source: String,
}

#[allow(dead_code)]
impl ScxmlDocument {
    /// Parse an SCXML document from a file path.
    ///
    /// This reads the file into memory and then delegates to
    /// [`ScxmlDocument::from_str`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// use nivasa_statechart::ScxmlDocument;
    ///
    /// let doc = ScxmlDocument::from_file("statecharts/app.scxml").unwrap();
    /// assert!(doc.metadata.name.is_some());
    /// ```
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ParseError> {
        let source = std::fs::read_to_string(path)?;
        Self::from_str(&source)
    }

    /// Parse an SCXML document from a string.
    ///
    /// The parser accepts well-formed SCXML and preserves the document shape
    /// for later validation and code generation.
    ///
    /// # Example
    ///
    /// ```rust
    /// use nivasa_statechart::ScxmlDocument;
    ///
    /// let scxml = r#"<?xml version="1.0" encoding="UTF-8"?>
    /// <scxml xmlns="http://www.w3.org/2005/07/scxml" version="1.0" initial="idle">
    ///   <state id="idle">
    ///     <transition event="go" target="running"/>
    ///   </state>
    ///   <state id="running"/>
    /// </scxml>"#;
    ///
    /// let doc = ScxmlDocument::from_str(scxml).unwrap();
    /// assert_eq!(doc.states["idle"].transitions[0].event.as_deref(), Some("go"));
    /// ```
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(source: &str) -> Result<Self, ParseError> {
        let mut reader = Reader::from_str(source);
        reader.config_mut().trim_text(true);

        let mut metadata = None;
        let mut states: HashMap<StateId, State> = HashMap::new();
        let mut top_level_states = Vec::new();
        let mut history_states = Vec::new();
        // Stack of parent state IDs for nested parsing
        let mut parent_stack: Vec<StateId> = Vec::new();
        // Track which element we're inside for transition parsing
        let mut current_state_id: Option<StateId> = None;
        let mut _in_onentry = false;
        let mut _in_onexit = false;

        let mut buf = Vec::new();

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(XmlEvent::Start(ref e)) => {
                    let tag_name = std::str::from_utf8(e.name().as_ref())?.to_string();
                    Self::handle_element(
                        &tag_name,
                        e,
                        false, // is_empty = false (Start elements have children)
                        &mut metadata,
                        &mut states,
                        &mut top_level_states,
                        &mut history_states,
                        &mut parent_stack,
                        &mut current_state_id,
                        &mut _in_onentry,
                        &mut _in_onexit,
                    )?;
                }
                Ok(XmlEvent::Empty(ref e)) => {
                    let tag_name = std::str::from_utf8(e.name().as_ref())?.to_string();
                    Self::handle_element(
                        &tag_name,
                        e,
                        true, // is_empty = true (self-closing, no children)
                        &mut metadata,
                        &mut states,
                        &mut top_level_states,
                        &mut history_states,
                        &mut parent_stack,
                        &mut current_state_id,
                        &mut _in_onentry,
                        &mut _in_onexit,
                    )?;
                }
                Ok(XmlEvent::End(ref e)) => {
                    let tag_name = std::str::from_utf8(e.name().as_ref())?.to_string();
                    match tag_name.as_str() {
                        "state" | "parallel" => {
                            parent_stack.pop();
                            current_state_id = parent_stack.last().cloned();
                        }
                        "onentry" => _in_onentry = false,
                        "onexit" => _in_onexit = false,
                        _ => {}
                    }
                }
                Ok(XmlEvent::Eof) => break,
                Err(e) => return Err(ParseError::Xml(e)),
                _ => {}
            }
            buf.clear();
        }

        let metadata = metadata
            .ok_or_else(|| ParseError::Invalid("Missing <scxml> root element".to_string()))?;

        Ok(ScxmlDocument {
            metadata,
            states,
            top_level_states,
            history_states,
            raw_source: source.to_string(),
        })
    }

    /// Handle a single XML element (Start or Empty).
    /// When `is_empty` is true, state/parallel elements do NOT push onto `parent_stack`
    /// since self-closing elements won't have a matching End event.
    #[allow(clippy::too_many_arguments)]
    fn handle_element(
        tag_name: &str,
        e: &BytesStart,
        is_empty: bool,
        metadata: &mut Option<ScxmlMetadata>,
        states: &mut HashMap<StateId, State>,
        top_level_states: &mut Vec<StateId>,
        history_states: &mut Vec<HistoryState>,
        parent_stack: &mut Vec<StateId>,
        current_state_id: &mut Option<StateId>,
        in_onentry: &mut bool,
        in_onexit: &mut bool,
    ) -> Result<(), ParseError> {
        match tag_name {
            "scxml" => {
                *metadata = Some(parse_scxml_attrs(e)?);
            }
            "state" => {
                let id =
                    get_attr(e, "id")?.unwrap_or_else(|| format!("__anonymous_{}", states.len()));
                let initial = get_attr(e, "initial")?;
                let parent = parent_stack.last().cloned();

                if let Some(ref parent_id) = parent {
                    if let Some(parent_state) = states.get_mut(parent_id) {
                        parent_state.children.push(id.clone());
                        if parent_state.state_type == StateType::Atomic {
                            parent_state.state_type = StateType::Compound;
                        }
                    }
                } else {
                    top_level_states.push(id.clone());
                }

                let state = State {
                    id: id.clone(),
                    state_type: StateType::Atomic,
                    parent,
                    children: Vec::new(),
                    initial,
                    transitions: Vec::new(),
                    has_on_entry: false,
                    has_on_exit: false,
                    invoke: Vec::new(),
                };
                states.insert(id.clone(), state);
                if !is_empty {
                    parent_stack.push(id.clone());
                    *current_state_id = Some(id);
                } else {
                    *current_state_id = parent_stack.last().cloned();
                }
            }
            "parallel" => {
                let id =
                    get_attr(e, "id")?.unwrap_or_else(|| format!("__parallel_{}", states.len()));
                let parent = parent_stack.last().cloned();

                if let Some(ref parent_id) = parent {
                    if let Some(parent_state) = states.get_mut(parent_id) {
                        parent_state.children.push(id.clone());
                        if parent_state.state_type == StateType::Atomic {
                            parent_state.state_type = StateType::Compound;
                        }
                    }
                } else {
                    top_level_states.push(id.clone());
                }

                let state = State {
                    id: id.clone(),
                    state_type: StateType::Parallel,
                    parent,
                    children: Vec::new(),
                    initial: None,
                    transitions: Vec::new(),
                    has_on_entry: false,
                    has_on_exit: false,
                    invoke: Vec::new(),
                };
                states.insert(id.clone(), state);
                if !is_empty {
                    parent_stack.push(id.clone());
                    *current_state_id = Some(id);
                } else {
                    *current_state_id = parent_stack.last().cloned();
                }
            }
            "final" => {
                let id = get_attr(e, "id")?.unwrap_or_else(|| format!("__final_{}", states.len()));
                let parent = parent_stack.last().cloned();

                if let Some(ref parent_id) = parent {
                    if let Some(parent_state) = states.get_mut(parent_id) {
                        parent_state.children.push(id.clone());
                        if parent_state.state_type == StateType::Atomic {
                            parent_state.state_type = StateType::Compound;
                        }
                    }
                } else {
                    top_level_states.push(id.clone());
                }

                let state = State {
                    id: id.clone(),
                    state_type: StateType::Final,
                    parent,
                    children: Vec::new(),
                    initial: None,
                    transitions: Vec::new(),
                    has_on_entry: false,
                    has_on_exit: false,
                    invoke: Vec::new(),
                };
                states.insert(id.clone(), state);
            }
            "transition" => {
                let transition = parse_transition(e)?;
                if let Some(ref state_id) = current_state_id {
                    if let Some(state) = states.get_mut(state_id.as_str()) {
                        state.transitions.push(transition);
                    }
                }
            }
            "onentry" => {
                *in_onentry = true;
                if let Some(ref state_id) = current_state_id {
                    if let Some(state) = states.get_mut(state_id.as_str()) {
                        state.has_on_entry = true;
                    }
                }
            }
            "onexit" => {
                *in_onexit = true;
                if let Some(ref state_id) = current_state_id {
                    if let Some(state) = states.get_mut(state_id.as_str()) {
                        state.has_on_exit = true;
                    }
                }
            }
            "history" => {
                let id = get_attr(e, "id")?.unwrap_or_default();
                let htype = match get_attr(e, "type")?.as_deref() {
                    Some("deep") => HistoryType::Deep,
                    _ => HistoryType::Shallow,
                };
                let parent = parent_stack.last().cloned().unwrap_or_default();
                history_states.push(HistoryState {
                    id,
                    history_type: htype,
                    parent,
                });
            }
            "invoke" => {
                let invoke = Invoke {
                    invoke_type: get_attr(e, "type")?,
                    id: get_attr(e, "id")?,
                    src: get_attr(e, "src")?,
                    autoforward: get_attr(e, "autoforward")?
                        .map(|v| v == "true")
                        .unwrap_or(false),
                };
                if let Some(ref state_id) = current_state_id {
                    if let Some(state) = states.get_mut(state_id.as_str()) {
                        state.invoke.push(invoke);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Get all state IDs in the document.
    pub fn state_ids(&self) -> Vec<&StateId> {
        self.states.keys().collect()
    }

    /// Get all unique event names used in transitions.
    pub fn event_names(&self) -> Vec<String> {
        let mut events: Vec<String> = self
            .states
            .values()
            .flat_map(|s| &s.transitions)
            .filter_map(|t| t.event.clone())
            .collect();
        events.sort();
        events.dedup();
        events
    }

    /// Check if a state ID exists in this document.
    pub fn has_state(&self, id: &str) -> bool {
        self.states.contains_key(id)
    }

    /// Compute a SHA-256 hash of the raw source for parity checking.
    pub fn content_hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.raw_source.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

fn parse_scxml_attrs(e: &BytesStart) -> Result<ScxmlMetadata, ParseError> {
    Ok(ScxmlMetadata {
        name: get_attr(e, "name")?,
        version: get_attr(e, "version")?.unwrap_or_else(|| "1.0".to_string()),
        initial: get_attr(e, "initial")?,
        datamodel: get_attr(e, "datamodel")?,
    })
}

fn parse_transition(e: &BytesStart) -> Result<Transition, ParseError> {
    let event = get_attr(e, "event")?;
    let cond = get_attr(e, "cond")?;
    let target = get_attr(e, "target")?
        .map(|t| t.split_whitespace().map(String::from).collect())
        .unwrap_or_default();
    let transition_type = match get_attr(e, "type")?.as_deref() {
        Some("internal") => TransitionType::Internal,
        _ => TransitionType::External,
    };

    Ok(Transition {
        event,
        cond,
        target,
        transition_type,
    })
}

fn get_attr(e: &BytesStart, name: &str) -> Result<Option<String>, ParseError> {
    for attr in e.attributes() {
        let attr = attr?;
        if attr.key.as_ref() == name.as_bytes() {
            let value = std::str::from_utf8(&attr.value)?.to_string();
            return Ok(Some(value));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_statechart() {
        let scxml = r#"<?xml version="1.0" encoding="UTF-8"?>
<scxml xmlns="http://www.w3.org/2005/07/scxml" version="1.0" name="test" initial="idle">
  <state id="idle">
    <transition event="start" target="running"/>
  </state>
  <state id="running">
    <transition event="stop" target="stopped"/>
  </state>
  <final id="stopped"/>
</scxml>"#;

        let doc = ScxmlDocument::from_str(scxml).unwrap();
        assert_eq!(doc.metadata.name, Some("test".to_string()));
        assert_eq!(doc.metadata.initial, Some("idle".to_string()));
        assert_eq!(doc.states.len(), 3);
        assert!(doc.has_state("idle"));
        assert!(doc.has_state("running"));
        assert!(doc.has_state("stopped"));

        let idle = &doc.states["idle"];
        assert_eq!(idle.state_type, StateType::Atomic);
        assert_eq!(idle.transitions.len(), 1);
        assert_eq!(idle.transitions[0].event, Some("start".to_string()));
        assert_eq!(idle.transitions[0].target, vec!["running"]);

        let stopped = &doc.states["stopped"];
        assert_eq!(stopped.state_type, StateType::Final);
    }

    #[test]
    fn test_parse_compound_state() {
        let scxml = r#"<?xml version="1.0" encoding="UTF-8"?>
<scxml xmlns="http://www.w3.org/2005/07/scxml" version="1.0" initial="parent">
  <state id="parent" initial="child1">
    <state id="child1">
      <transition event="next" target="child2"/>
    </state>
    <final id="child2"/>
  </state>
</scxml>"#;

        let doc = ScxmlDocument::from_str(scxml).unwrap();
        let parent = &doc.states["parent"];
        assert_eq!(parent.state_type, StateType::Compound);
        assert_eq!(parent.children, vec!["child1", "child2"]);
        assert_eq!(parent.initial, Some("child1".to_string()));

        let child1 = &doc.states["child1"];
        assert_eq!(child1.parent, Some("parent".to_string()));
    }

    #[test]
    fn test_parse_parallel_state() {
        let scxml = r#"<?xml version="1.0" encoding="UTF-8"?>
<scxml xmlns="http://www.w3.org/2005/07/scxml" version="1.0" initial="p">
  <parallel id="p">
    <state id="region1"/>
    <state id="region2"/>
  </parallel>
</scxml>"#;

        let doc = ScxmlDocument::from_str(scxml).unwrap();
        let p = &doc.states["p"];
        assert_eq!(p.state_type, StateType::Parallel);
        assert_eq!(p.children.len(), 2);
    }

    #[test]
    fn test_event_names_extraction() {
        let scxml = r#"<?xml version="1.0" encoding="UTF-8"?>
<scxml xmlns="http://www.w3.org/2005/07/scxml" version="1.0" initial="a">
  <state id="a">
    <transition event="go" target="b"/>
    <transition event="error.fatal" target="c"/>
  </state>
  <state id="b">
    <transition event="go" target="c"/>
  </state>
  <final id="c"/>
</scxml>"#;

        let doc = ScxmlDocument::from_str(scxml).unwrap();
        let events = doc.event_names();
        assert_eq!(events, vec!["error.fatal", "go"]);
    }

    #[test]
    fn test_content_hash_deterministic() {
        let scxml =
            r#"<?xml version="1.0"?><scxml version="1.0" initial="a"><state id="a"/></scxml>"#;
        let doc1 = ScxmlDocument::from_str(scxml).unwrap();
        let doc2 = ScxmlDocument::from_str(scxml).unwrap();
        assert_eq!(doc1.content_hash(), doc2.content_hash());
    }

    #[test]
    fn test_onentry_detection() {
        let scxml = r#"<?xml version="1.0" encoding="UTF-8"?>
<scxml xmlns="http://www.w3.org/2005/07/scxml" version="1.0" initial="a">
  <state id="a">
    <onentry>
      <log expr="'entering a'"/>
    </onentry>
    <transition event="next" target="b"/>
  </state>
  <state id="b"/>
</scxml>"#;

        let doc = ScxmlDocument::from_str(scxml).unwrap();
        assert!(doc.states["a"].has_on_entry);
        assert!(!doc.states["b"].has_on_entry);
    }
}
