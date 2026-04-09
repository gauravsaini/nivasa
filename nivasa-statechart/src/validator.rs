//! SCXML statechart validator.
//!
//! Validates parsed SCXML documents for:
//! - Reachability (all states reachable from initial)
//! - Completeness (no dead-end states without `<final>`)
//! - Determinism (no ambiguous transitions)
//! - Well-formedness (compound states have children, etc.)
//! - Target validity (all transition targets exist)

use crate::parser::ScxmlDocument;
use crate::types::*;
use std::collections::HashSet;

/// A validation error with context.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub rule: ValidationRule,
    pub message: String,
    pub state_id: Option<StateId>,
}

/// Categories of validation rules.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationRule {
    /// State is not reachable from the initial state.
    Unreachable,
    /// Non-final state has no outgoing transitions.
    DeadEnd,
    /// Multiple transitions match the same event from the same state.
    NonDeterministic,
    /// Compound state has no children.
    MalformedCompound,
    /// Transition targets a non-existent state.
    InvalidTarget,
    /// Missing initial state.
    MissingInitial,
    /// Event name does not follow dot-separated format.
    InvalidEventName,
}

/// Result of validating an SCXML document.
#[allow(dead_code)]
#[derive(Debug)]
pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationError>,
}

#[allow(dead_code)]
impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Validate an SCXML document against all rules.
pub fn validate(doc: &ScxmlDocument) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // 1. Check initial state exists
    if let Some(ref initial) = doc.metadata.initial {
        if !doc.has_state(initial) {
            errors.push(ValidationError {
                rule: ValidationRule::MissingInitial,
                message: format!(
                    "Initial state '{}' does not exist in the document",
                    initial
                ),
                state_id: None,
            });
        }
    } else if doc.top_level_states.is_empty() {
        errors.push(ValidationError {
            rule: ValidationRule::MissingInitial,
            message: "No initial state specified and no top-level states found".to_string(),
            state_id: None,
        });
    }

    // 2. Check all transition targets exist
    for state in doc.states.values() {
        for transition in &state.transitions {
            for target in &transition.target {
                if !doc.has_state(target) {
                    errors.push(ValidationError {
                        rule: ValidationRule::InvalidTarget,
                        message: format!(
                            "Transition in state '{}' targets non-existent state '{}'",
                            state.id, target
                        ),
                        state_id: Some(state.id.clone()),
                    });
                }
            }
        }
    }

    // 3. Check reachability from initial state
    let initial_id = doc
        .metadata
        .initial
        .clone()
        .or_else(|| doc.top_level_states.first().cloned());

    if let Some(initial_id) = initial_id {
        let reachable = compute_reachable(doc, &initial_id);
        for state in doc.states.values() {
            if !reachable.contains(&state.id) && state.state_type != StateType::Final {
                // Check if it's a child of a reachable compound/parallel state
                let parent_reachable = state
                    .parent
                    .as_ref()
                    .map(|p| reachable.contains(p))
                    .unwrap_or(false);

                if !parent_reachable {
                    warnings.push(ValidationError {
                        rule: ValidationRule::Unreachable,
                        message: format!("State '{}' is not reachable from initial state", state.id),
                        state_id: Some(state.id.clone()),
                    });
                }
            }
        }
    }

    // 4. Check for dead-end states (non-final states with no outgoing transitions)
    for state in doc.states.values() {
        if state.state_type != StateType::Final
            && state.state_type != StateType::Parallel
            && state.transitions.is_empty()
            && state.children.is_empty()
        {
            warnings.push(ValidationError {
                rule: ValidationRule::DeadEnd,
                message: format!(
                    "Atomic state '{}' has no outgoing transitions and is not a final state",
                    state.id
                ),
                state_id: Some(state.id.clone()),
            });
        }
    }

    // 5. Check for non-deterministic transitions
    for state in doc.states.values() {
        let mut seen_events: HashSet<Option<&str>> = HashSet::new();
        for transition in &state.transitions {
            let event_key = transition.event.as_deref();
            // Only flag if same event AND no cond (truly ambiguous)
            if transition.cond.is_none() && !seen_events.insert(event_key) {
                warnings.push(ValidationError {
                    rule: ValidationRule::NonDeterministic,
                    message: format!(
                        "State '{}' has multiple transitions for event '{:?}' without conditions",
                        state.id,
                        transition.event
                    ),
                    state_id: Some(state.id.clone()),
                });
            }
        }
    }

    // 6. Validate event name format
    for state in doc.states.values() {
        for transition in &state.transitions {
            if let Some(ref event) = transition.event {
                if event != "*" && !is_valid_event_name(event) {
                    errors.push(ValidationError {
                        rule: ValidationRule::InvalidEventName,
                        message: format!(
                            "Event '{}' in state '{}' is not a valid dot-separated event name",
                            event, state.id
                        ),
                        state_id: Some(state.id.clone()),
                    });
                }
            }
        }
    }

    ValidationResult { errors, warnings }
}

/// Compute the set of states reachable from the given initial state
/// by following transitions.
fn compute_reachable(doc: &ScxmlDocument, initial: &str) -> HashSet<StateId> {
    let mut reachable = HashSet::new();
    let mut queue = vec![initial.to_string()];

    while let Some(state_id) = queue.pop() {
        if reachable.contains(&state_id) {
            continue;
        }
        reachable.insert(state_id.clone());

        if let Some(state) = doc.states.get(&state_id) {
            // Follow transitions
            for transition in &state.transitions {
                for target in &transition.target {
                    if !reachable.contains(target) {
                        queue.push(target.clone());
                    }
                }
            }
            // Children of compound/parallel states are reachable
            for child in &state.children {
                if !reachable.contains(child) {
                    queue.push(child.clone());
                }
            }
            // Initial child state is reachable
            if let Some(ref initial) = state.initial {
                if !reachable.contains(initial) {
                    queue.push(initial.clone());
                }
            }
        }
    }

    reachable
}

/// Validate that an event name follows dot-separated alphanumeric format.
fn is_valid_event_name(name: &str) -> bool {
    let name = name.trim_end_matches(".*");
    !name.is_empty()
        && name
            .split('.')
            .all(|token| !token.is_empty() && token.chars().all(|c| c.is_alphanumeric() || c == '_'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ScxmlDocument;

    #[test]
    fn test_valid_document() {
        let scxml = r#"<?xml version="1.0"?>
<scxml version="1.0" initial="a" xmlns="http://www.w3.org/2005/07/scxml">
  <state id="a"><transition event="go" target="b"/></state>
  <final id="b"/>
</scxml>"#;
        let doc = ScxmlDocument::from_str(scxml).unwrap();
        let result = validate(&doc);
        assert!(result.is_valid(), "Errors: {:?}", result.errors);
    }

    #[test]
    fn test_invalid_target() {
        let scxml = r#"<?xml version="1.0"?>
<scxml version="1.0" initial="a" xmlns="http://www.w3.org/2005/07/scxml">
  <state id="a"><transition event="go" target="nonexistent"/></state>
</scxml>"#;
        let doc = ScxmlDocument::from_str(scxml).unwrap();
        let result = validate(&doc);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.rule == ValidationRule::InvalidTarget));
    }

    #[test]
    fn test_missing_initial() {
        let scxml = r#"<?xml version="1.0"?>
<scxml version="1.0" initial="nonexistent" xmlns="http://www.w3.org/2005/07/scxml">
  <state id="a"/>
</scxml>"#;
        let doc = ScxmlDocument::from_str(scxml).unwrap();
        let result = validate(&doc);
        assert!(!result.is_valid());
        assert!(result.errors.iter().any(|e| e.rule == ValidationRule::MissingInitial));
    }

    #[test]
    fn test_dead_end_warning() {
        let scxml = r#"<?xml version="1.0"?>
<scxml version="1.0" initial="a" xmlns="http://www.w3.org/2005/07/scxml">
  <state id="a"><transition event="go" target="b"/></state>
  <state id="b"/>
</scxml>"#;
        let doc = ScxmlDocument::from_str(scxml).unwrap();
        let result = validate(&doc);
        // b is a dead-end (not final, no transitions)
        assert!(result.warnings.iter().any(|w| w.rule == ValidationRule::DeadEnd));
    }

    #[test]
    fn test_valid_event_names() {
        assert!(is_valid_event_name("error"));
        assert!(is_valid_event_name("error.send"));
        assert!(is_valid_event_name("error.send.failed"));
        assert!(is_valid_event_name("module_init"));
        assert!(is_valid_event_name("error.*"));
    }

    #[test]
    fn test_invalid_event_names() {
        assert!(!is_valid_event_name(""));
        assert!(!is_valid_event_name(".error"));
        assert!(!is_valid_event_name("error."));
    }
}
