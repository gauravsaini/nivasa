//! Core SCXML types derived from the W3C specification.
//!
//! These types represent the in-memory model of an SCXML document,
//! closely following the spec's element definitions.

use serde::{Deserialize, Serialize};

/// Unique identifier for a state within a statechart.
pub type StateId = String;

/// The type of a state node in the statechart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StateType {
    /// An atomic state — a `<state>` with no child states.
    Atomic,
    /// A compound state — a `<state>` with child states.
    Compound,
    /// A parallel state — `<parallel>`, all children are simultaneously active.
    Parallel,
    /// A final state — `<final>`, reaching this terminates the parent region.
    Final,
}

/// A state node in the statechart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    /// Unique identifier for this state.
    pub id: StateId,
    /// The type of this state node.
    pub state_type: StateType,
    /// Parent state ID, if any. `None` for top-level states.
    pub parent: Option<StateId>,
    /// Child state IDs (for compound and parallel states).
    pub children: Vec<StateId>,
    /// The initial child state (for compound states).
    pub initial: Option<StateId>,
    /// Outgoing transitions from this state.
    pub transitions: Vec<Transition>,
    /// Whether this state has onentry actions.
    pub has_on_entry: bool,
    /// Whether this state has onexit actions.
    pub has_on_exit: bool,
    /// Invocations of external services from this state.
    pub invoke: Vec<Invoke>,
}

/// The type of a transition (internal vs external).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionType {
    /// Internal transition — source state is NOT exited if target is a descendant.
    Internal,
    /// External transition — source state is always exited.
    External,
}

/// A transition between states, triggered by events and guarded by conditions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    /// Event descriptor(s) that trigger this transition. `None` = eventless transition.
    pub event: Option<String>,
    /// Guard condition expression. `None` = always true.
    pub cond: Option<String>,
    /// Target state ID(s). Empty = targetless transition.
    pub target: Vec<StateId>,
    /// Whether this is an internal or external transition.
    pub transition_type: TransitionType,
}

/// History pseudo-state type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoryType {
    /// Shallow history — remembers only the immediate child states.
    Shallow,
    /// Deep history — remembers the full nested state configuration.
    Deep,
}

/// A history pseudo-state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryState {
    pub id: StateId,
    pub history_type: HistoryType,
    pub parent: StateId,
}

/// An invocation of an external service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invoke {
    /// Type of service to invoke.
    pub invoke_type: Option<String>,
    /// Unique identifier for this invocation.
    pub id: Option<String>,
    /// Source URI for the invoked service.
    pub src: Option<String>,
    /// Whether to forward events to the invoked service.
    pub autoforward: bool,
}

/// The type of an SCXML event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    /// Internal event (raised by `<raise>` or platform).
    Internal,
    /// External event (from `<send>` or external sources).
    External,
    /// Platform-generated event (errors, done events).
    Platform,
}

/// An SCXML event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Hierarchical event name (dot-separated, e.g., "error.module.circular").
    pub name: String,
    /// The type of event.
    pub event_type: EventType,
    /// Optional event data payload.
    #[serde(skip)]
    pub data: Option<serde_json::Value>,
    /// Origin of the event (for external events).
    pub origin: Option<String>,
    /// ID of the invoke that generated this event.
    pub invoke_id: Option<String>,
}

impl Event {
    /// Create a new internal event with the given name.
    pub fn internal(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            event_type: EventType::Internal,
            data: None,
            origin: None,
            invoke_id: None,
        }
    }

    /// Create a new external event with the given name.
    pub fn external(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            event_type: EventType::External,
            data: None,
            origin: None,
            invoke_id: None,
        }
    }

    /// Attach data to this event.
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }

    /// Check if this event matches an event descriptor (SCXML prefix matching).
    ///
    /// Per the W3C spec: an event descriptor matches an event name if its string
    /// of tokens is an exact match or a prefix of the event's name tokens.
    /// Example: descriptor "error" matches "error", "error.send", "error.send.failed".
    pub fn matches_descriptor(&self, descriptor: &str) -> bool {
        if descriptor == "*" {
            return true;
        }

        let descriptor = descriptor.trim_end_matches(".*").trim_end_matches('.');
        let desc_tokens: Vec<&str> = descriptor.split('.').collect();
        let name_tokens: Vec<&str> = self.name.split('.').collect();

        if desc_tokens.len() > name_tokens.len() {
            return false;
        }

        desc_tokens
            .iter()
            .zip(name_tokens.iter())
            .all(|(d, n)| d == n)
    }
}

/// Metadata about the parsed SCXML document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScxmlMetadata {
    /// Name of the statechart.
    pub name: Option<String>,
    /// SCXML version (must be "1.0").
    pub version: String,
    /// Initial state ID.
    pub initial: Option<StateId>,
    /// Data model type.
    pub datamodel: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_matches_exact() {
        let event = Event::internal("error.send.failed");
        assert!(event.matches_descriptor("error.send.failed"));
    }

    #[test]
    fn test_event_matches_prefix() {
        let event = Event::internal("error.send.failed");
        assert!(event.matches_descriptor("error"));
        assert!(event.matches_descriptor("error.send"));
    }

    #[test]
    fn test_event_matches_wildcard() {
        let event = Event::internal("error.send.failed");
        assert!(event.matches_descriptor("*"));
        assert!(event.matches_descriptor("error.*"));
    }

    #[test]
    fn test_event_no_match() {
        let event = Event::internal("error.send.failed");
        assert!(!event.matches_descriptor("errors"));
        assert!(!event.matches_descriptor("error.receive"));
        assert!(!event.matches_descriptor("err"));
    }

    #[test]
    fn test_event_no_match_longer_descriptor() {
        let event = Event::internal("error");
        assert!(!event.matches_descriptor("error.send"));
    }
}
