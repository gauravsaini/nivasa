//! Statechart runtime engine.
//!
//! The `StatechartEngine` is the **only** mechanism for state transitions at runtime.
//! There is no `set_state()` — all transitions go through `send_event()`.
//!
//! Invalid transitions are:
//! - **Debug builds**: panic with full diagnostic
//! - **Release builds**: return `Err(InvalidTransitionError)`
//!
//! ```rust
//! use nivasa_statechart::engine::{StatechartEngine, StatechartSpec};
//!
//! #[derive(Debug, Clone, Copy, PartialEq, Eq)]
//! enum DemoState {
//!     Idle,
//!     Done,
//! }
//!
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! enum DemoEvent {
//!     Finish,
//! }
//!
//! struct DemoSpec;
//!
//! impl StatechartSpec for DemoSpec {
//!     type State = DemoState;
//!     type Event = DemoEvent;
//!
//!     fn transition(current: &Self::State, event: &Self::Event) -> Option<Self::State> {
//!         match (current, event) {
//!             (DemoState::Idle, DemoEvent::Finish) => Some(DemoState::Done),
//!             _ => None,
//!         }
//!     }
//!
//!     fn valid_events_for(state: &Self::State) -> Vec<Self::Event> {
//!         match state {
//!             DemoState::Idle => vec![DemoEvent::Finish],
//!             DemoState::Done => vec![],
//!         }
//!     }
//!
//!     fn is_final(state: &Self::State) -> bool {
//!         matches!(state, DemoState::Done)
//!     }
//!
//!     fn name() -> &'static str {
//!         "demo"
//!     }
//!
//!     fn scxml_hash() -> &'static str {
//!         "hash"
//!     }
//! }
//!
//! let mut engine = StatechartEngine::<DemoSpec>::new(DemoState::Idle);
//! assert_eq!(engine.current_state(), DemoState::Idle);
//! assert_eq!(engine.send_event(DemoEvent::Finish).unwrap(), DemoState::Done);
//! assert!(engine.is_in_final_state());
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;
use std::marker::PhantomData;

#[cfg(debug_assertions)]
use std::collections::VecDeque;

#[cfg(debug_assertions)]
const MAX_RECENT_TRANSITIONS: usize = 64;

/// Trait that generated code must implement for each statechart.
///
/// This ties together the State enum, Event enum, transition function,
/// and handler trait for a specific statechart.
pub trait StatechartSpec: Send + Sync + 'static {
    /// The state enum (generated from SCXML `<state>` elements).
    type State: fmt::Debug + Clone + Copy + PartialEq + Eq + Send + Sync;
    /// The event enum (generated from SCXML transition `event` attributes).
    type Event: fmt::Debug + Clone + PartialEq + Eq + Send + Sync;

    /// The generated transition function. Returns `Some(target)` if the
    /// transition is valid, `None` otherwise.
    fn transition(current: &Self::State, event: &Self::Event) -> Option<Self::State>;

    /// Returns the list of valid events for the given state.
    fn valid_events_for(state: &Self::State) -> Vec<Self::Event>;

    /// Resolve a state to the effective entered leaf state.
    ///
    /// Compound states may specify an initial child in SCXML. Generated specs
    /// override this method so the runtime lands on the correct entered state
    /// instead of exposing an intermediate container state.
    fn enter_initial_state(state: Self::State) -> Self::State {
        state
    }

    /// Returns `true` if the state is a final state.
    fn is_final(state: &Self::State) -> bool;

    /// The name of this statechart (from `<scxml name="...">`)
    fn name() -> &'static str;

    /// The SCXML content hash (for parity checking).
    fn scxml_hash() -> &'static str;
}

/// Callback trait for observing state transitions.
pub trait StatechartTracer: Send + Sync {
    /// Called after every valid state transition.
    fn on_transition(&self, from: &str, event: &str, to: &str);
    /// Called when an invalid transition is attempted.
    fn on_invalid_transition(&self, from: &str, event: &str, valid_events: &[String]);
}

/// Built-in tracer that emits transition events via `tracing`.
#[derive(Debug, Default, Clone, Copy)]
pub struct LoggingTracer;

impl StatechartTracer for LoggingTracer {
    fn on_transition(&self, from: &str, event: &str, to: &str) {
        tracing::debug!(from, event, to, "statechart transition");
    }

    fn on_invalid_transition(&self, from: &str, event: &str, valid_events: &[String]) {
        tracing::warn!(from, event, ?valid_events, "invalid statechart transition");
    }
}

/// Kind of transition captured in debug history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionKind {
    Valid,
    Invalid,
}

/// A serializable record of one transition attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionRecord {
    pub kind: TransitionKind,
    pub from: String,
    pub event: String,
    pub to: Option<String>,
    pub valid_events: Vec<String>,
}

impl TransitionRecord {
    pub fn valid(from: impl Into<String>, event: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            kind: TransitionKind::Valid,
            from: from.into(),
            event: event.into(),
            to: Some(to.into()),
            valid_events: Vec::new(),
        }
    }

    pub fn invalid(
        from: impl Into<String>,
        event: impl Into<String>,
        valid_events: Vec<String>,
    ) -> Self {
        Self {
            kind: TransitionKind::Invalid,
            from: from.into(),
            event: event.into(),
            to: None,
            valid_events,
        }
    }
}

/// Serializable snapshot of an engine for debug inspection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatechartSnapshot {
    pub statechart_name: String,
    pub current_state: String,
    pub scxml_hash: String,
    pub raw_scxml: Option<String>,
    pub recent_transitions: Vec<TransitionRecord>,
}

/// Error returned when an invalid transition is attempted (release builds).
pub struct InvalidTransitionError<S: StatechartSpec> {
    pub current_state: S::State,
    pub event: S::Event,
    pub valid_events: Vec<S::Event>,
}

impl<S: StatechartSpec> fmt::Debug for InvalidTransitionError<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InvalidTransitionError")
            .field("current_state", &self.current_state)
            .field("event", &self.event)
            .field("valid_events", &self.valid_events)
            .finish()
    }
}

impl<S: StatechartSpec> fmt::Display for InvalidTransitionError<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SCXML violation in statechart '{}': no transition from {:?} for event {:?}. \
             Valid events: {:?}",
            S::name(),
            self.current_state,
            self.event,
            self.valid_events,
        )
    }
}

impl<S: StatechartSpec> std::error::Error for InvalidTransitionError<S> {}

/// The statechart runtime engine.
///
/// This is the **only way to transition state**. There is no `set_state()`.
/// The engine validates every transition against the generated transition table.
pub struct StatechartEngine<S: StatechartSpec> {
    /// Current state — private, no public setter.
    current_state: S::State,
    /// Optional tracer for observing transitions.
    tracer: Option<Box<dyn StatechartTracer>>,
    #[cfg(debug_assertions)]
    recent_transitions: VecDeque<TransitionRecord>,
    /// Phantom for spec type.
    _spec: PhantomData<S>,
}

impl<S: StatechartSpec> StatechartEngine<S> {
    /// Create a new engine in the given initial state.
    ///
    /// The engine resolves any generated SCXML initial-child state before it
    /// stores the starting state.
    ///
    /// ```rust
    /// use nivasa_statechart::engine::{StatechartEngine, StatechartSpec};
    ///
    /// # #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    /// # enum DemoState { Idle }
    /// # #[derive(Debug, Clone, PartialEq, Eq)]
    /// # enum DemoEvent { Finish }
    /// # struct DemoSpec;
    /// # impl StatechartSpec for DemoSpec {
    /// #     type State = DemoState;
    /// #     type Event = DemoEvent;
    /// #     fn transition(_: &Self::State, _: &Self::Event) -> Option<Self::State> { None }
    /// #     fn valid_events_for(_: &Self::State) -> Vec<Self::Event> { vec![] }
    /// #     fn is_final(_: &Self::State) -> bool { false }
    /// #     fn name() -> &'static str { "demo" }
    /// #     fn scxml_hash() -> &'static str { "hash" }
    /// # }
    /// let engine = StatechartEngine::<DemoSpec>::new(DemoState::Idle);
    /// assert_eq!(engine.current_state(), DemoState::Idle);
    /// ```
    pub fn new(initial_state: S::State) -> Self {
        Self {
            current_state: S::enter_initial_state(initial_state),
            tracer: None,
            #[cfg(debug_assertions)]
            recent_transitions: VecDeque::new(),
            _spec: PhantomData,
        }
    }

    /// Create a new engine with a tracer.
    ///
    /// Use this when you want transition visibility without changing runtime
    /// behavior.
    ///
    /// ```rust
    /// use nivasa_statechart::engine::{StatechartEngine, StatechartSpec, StatechartTracer};
    ///
    /// # #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    /// # enum DemoState { Idle, Done }
    /// # #[derive(Debug, Clone, PartialEq, Eq)]
    /// # enum DemoEvent { Finish }
    /// # struct DemoSpec;
    /// # impl StatechartSpec for DemoSpec {
    /// #     type State = DemoState;
    /// #     type Event = DemoEvent;
    /// #     fn transition(current: &Self::State, event: &Self::Event) -> Option<Self::State> {
    /// #         match (current, event) {
    /// #             (DemoState::Idle, DemoEvent::Finish) => Some(DemoState::Done),
    /// #             _ => None,
    /// #         }
    /// #     }
    /// #     fn valid_events_for(state: &Self::State) -> Vec<Self::Event> {
    /// #         match state {
    /// #             DemoState::Idle => vec![DemoEvent::Finish],
    /// #             DemoState::Done => vec![],
    /// #         }
    /// #     }
    /// #     fn is_final(state: &Self::State) -> bool { matches!(state, DemoState::Done) }
    /// #     fn name() -> &'static str { "demo" }
    /// #     fn scxml_hash() -> &'static str { "hash" }
    /// # }
    /// # #[derive(Default)]
    /// # struct NoopTracer;
    /// # impl StatechartTracer for NoopTracer {
    /// #     fn on_transition(&self, _: &str, _: &str, _: &str) {}
    /// #     fn on_invalid_transition(&self, _: &str, _: &str, _: &[String]) {}
    /// # }
    /// let mut engine = StatechartEngine::<DemoSpec>::with_tracer(
    ///     DemoState::Idle,
    ///     Box::new(NoopTracer::default()),
    /// );
    /// assert_eq!(engine.current_state(), DemoState::Idle);
    /// assert_eq!(engine.send_event(DemoEvent::Finish).unwrap(), DemoState::Done);
    /// ```
    pub fn with_tracer(initial_state: S::State, tracer: Box<dyn StatechartTracer>) -> Self {
        Self {
            current_state: S::enter_initial_state(initial_state),
            tracer: Some(tracer),
            #[cfg(debug_assertions)]
            recent_transitions: VecDeque::new(),
            _spec: PhantomData,
        }
    }

    /// The **only** public method that changes state.
    ///
    /// Validates the (current_state, event) pair against the transition table.
    /// On success: transitions to the target state and returns it.
    /// On failure (debug): **panics** with full diagnostic info.
    /// On failure (release): returns `Err(InvalidTransitionError)`.
    ///
    /// ```rust
    /// use nivasa_statechart::engine::{StatechartEngine, StatechartSpec};
    ///
    /// # #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    /// # enum DemoState { Idle, Done }
    /// # #[derive(Debug, Clone, PartialEq, Eq)]
    /// # enum DemoEvent { Finish }
    /// # struct DemoSpec;
    /// # impl StatechartSpec for DemoSpec {
    /// #     type State = DemoState;
    /// #     type Event = DemoEvent;
    /// #     fn transition(current: &Self::State, event: &Self::Event) -> Option<Self::State> {
    /// #         match (current, event) {
    /// #             (DemoState::Idle, DemoEvent::Finish) => Some(DemoState::Done),
    /// #             _ => None,
    /// #         }
    /// #     }
    /// #     fn valid_events_for(state: &Self::State) -> Vec<Self::Event> {
    /// #         match state {
    /// #             DemoState::Idle => vec![DemoEvent::Finish],
    /// #             DemoState::Done => vec![],
    /// #         }
    /// #     }
    /// #     fn is_final(state: &Self::State) -> bool { matches!(state, DemoState::Done) }
    /// #     fn name() -> &'static str { "demo" }
    /// #     fn scxml_hash() -> &'static str { "hash" }
    /// # }
    /// let mut engine = StatechartEngine::<DemoSpec>::new(DemoState::Idle);
    /// let next = engine.send_event(DemoEvent::Finish).unwrap();
    /// assert_eq!(next, DemoState::Done);
    /// ```
    pub fn send_event(
        &mut self,
        event: S::Event,
    ) -> Result<S::State, InvalidTransitionError<S>> {
        let from = format!("{:?}", self.current_state);
        let event_str = format!("{:?}", event);

        match S::transition(&self.current_state, &event) {
            Some(target) => {
                let target = S::enter_initial_state(target);
                let to = format!("{:?}", target);
                if let Some(ref tracer) = self.tracer {
                    tracer.on_transition(&from, &event_str, &to);
                }
                self.record_transition(TransitionRecord::valid(from, event_str, to));
                self.current_state = target;
                Ok(target)
            }
            None => {
                let valid_events = S::valid_events_for(&self.current_state);
                let valid_events_debug = valid_events
                    .iter()
                    .map(|e| format!("{:?}", e))
                    .collect::<Vec<_>>();

                if let Some(ref tracer) = self.tracer {
                    tracer.on_invalid_transition(&from, &event_str, &valid_events_debug);
                }

                self.record_transition(TransitionRecord::invalid(
                    from,
                    event_str,
                    valid_events_debug,
                ));

                let err: InvalidTransitionError<S> = InvalidTransitionError {
                    current_state: self.current_state,
                    event,
                    valid_events,
                };

                if cfg!(debug_assertions) {
                    panic!("{}", err);
                }

                Err(err)
            }
        }
    }

    /// Read-only access to the current state.
    pub fn current_state(&self) -> S::State {
        self.current_state
    }

    /// Check if the engine is in a final state.
    pub fn is_in_final_state(&self) -> bool {
        S::is_final(&self.current_state)
    }

    /// Get the list of valid events from the current state.
    pub fn valid_events(&self) -> Vec<S::Event> {
        S::valid_events_for(&self.current_state)
    }

    /// Get the name of the statechart.
    pub fn statechart_name(&self) -> &'static str {
        S::name()
    }

    /// Get the SCXML content hash for parity checking.
    pub fn scxml_hash(&self) -> &'static str {
        S::scxml_hash()
    }

    /// Return a serializable snapshot for debug inspection.
    ///
    /// The snapshot includes the statechart identity, current state, SCXML
    /// hash, optional raw SCXML, and recent transition history.
    ///
    /// ```rust
    /// use nivasa_statechart::engine::{StatechartEngine, StatechartSpec, StatechartTracer};
    ///
    /// # #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    /// # enum DemoState { Idle, Done }
    /// # #[derive(Debug, Clone, PartialEq, Eq)]
    /// # enum DemoEvent { Finish }
    /// # struct DemoSpec;
    /// # impl StatechartSpec for DemoSpec {
    /// #     type State = DemoState;
    /// #     type Event = DemoEvent;
    /// #     fn transition(current: &Self::State, event: &Self::Event) -> Option<Self::State> {
    /// #         match (current, event) {
    /// #             (DemoState::Idle, DemoEvent::Finish) => Some(DemoState::Done),
    /// #             _ => None,
    /// #         }
    /// #     }
    /// #     fn valid_events_for(state: &Self::State) -> Vec<Self::Event> {
    /// #         match state {
    /// #             DemoState::Idle => vec![DemoEvent::Finish],
    /// #             DemoState::Done => vec![],
    /// #         }
    /// #     }
    /// #     fn is_final(state: &Self::State) -> bool { matches!(state, DemoState::Done) }
    /// #     fn name() -> &'static str { "demo" }
    /// #     fn scxml_hash() -> &'static str { "hash" }
    /// # }
    /// # struct NoopTracer;
    /// # impl StatechartTracer for NoopTracer {
    /// #     fn on_transition(&self, _: &str, _: &str, _: &str) {}
    /// #     fn on_invalid_transition(&self, _: &str, _: &str, _: &[String]) {}
    /// # }
    /// let mut engine = StatechartEngine::<DemoSpec>::with_tracer(
    ///     DemoState::Idle,
    ///     Box::new(NoopTracer),
    /// );
    /// engine.send_event(DemoEvent::Finish).unwrap();
    ///
    /// let snapshot = engine.snapshot(Some("<scxml name=\"demo\"/>".to_string()));
    /// assert_eq!(snapshot.statechart_name, "demo");
    /// assert_eq!(snapshot.current_state, "Done");
    /// assert_eq!(snapshot.raw_scxml.as_deref(), Some("<scxml name=\"demo\"/>"));
    /// ```
    pub fn snapshot(&self, raw_scxml: Option<String>) -> StatechartSnapshot {
        StatechartSnapshot {
            statechart_name: S::name().to_string(),
            current_state: format!("{:?}", self.current_state),
            scxml_hash: S::scxml_hash().to_string(),
            raw_scxml,
            recent_transitions: self.recent_transitions(),
        }
    }

    #[cfg(debug_assertions)]
    fn record_transition(&mut self, record: TransitionRecord) {
        if self.recent_transitions.len() >= MAX_RECENT_TRANSITIONS {
            self.recent_transitions.pop_front();
        }
        self.recent_transitions.push_back(record);
    }

    #[cfg(not(debug_assertions))]
    fn record_transition(&mut self, _record: TransitionRecord) {}

    /// Return the recent transition history.
    ///
    /// In debug builds this returns a bounded list of valid and invalid
    /// transition attempts. In release builds it is empty.
    pub fn recent_transitions(&self) -> Vec<TransitionRecord> {
        #[cfg(debug_assertions)]
        {
            self.recent_transitions.iter().cloned().collect()
        }

        #[cfg(not(debug_assertions))]
        {
            Vec::new()
        }
    }
}

// NOTE: There is NO set_state(), NO force_transition(), NO backdoor.
// This is by design. The statechart engine IS the enforcement mechanism.

#[cfg(test)]
mod tests {
    use super::*;

    // Define a test statechart spec inline
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum TestState {
        Idle,
        Running,
        Done,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum TestEvent {
        Start,
        Finish,
    }

    struct TestSpec;

    impl StatechartSpec for TestSpec {
        type State = TestState;
        type Event = TestEvent;

        fn transition(current: &TestState, event: &TestEvent) -> Option<TestState> {
            match (current, event) {
                (TestState::Idle, TestEvent::Start) => Some(TestState::Running),
                (TestState::Running, TestEvent::Finish) => Some(TestState::Done),
                _ => None,
            }
        }

        fn valid_events_for(state: &TestState) -> Vec<TestEvent> {
            match state {
                TestState::Idle => vec![TestEvent::Start],
                TestState::Running => vec![TestEvent::Finish],
                TestState::Done => vec![],
            }
        }

        fn is_final(state: &TestState) -> bool {
            matches!(state, TestState::Done)
        }

        fn name() -> &'static str {
            "test"
        }

        fn scxml_hash() -> &'static str {
            "test_hash"
        }
    }

    #[test]
    fn test_valid_transitions() {
        let mut engine = StatechartEngine::<TestSpec>::new(TestState::Idle);
        assert_eq!(engine.current_state(), TestState::Idle);

        let next = engine.send_event(TestEvent::Start).unwrap();
        assert_eq!(next, TestState::Running);
        assert_eq!(engine.current_state(), TestState::Running);

        let next = engine.send_event(TestEvent::Finish).unwrap();
        assert_eq!(next, TestState::Done);
        assert!(engine.is_in_final_state());
    }

    #[test]
    #[should_panic(expected = "SCXML violation")]
    fn test_invalid_transition_panics_in_debug() {
        let mut engine = StatechartEngine::<TestSpec>::new(TestState::Idle);
        // Finish is not valid from Idle — should panic in debug mode
        let _ = engine.send_event(TestEvent::Finish);
    }

    #[test]
    fn test_valid_events() {
        let engine = StatechartEngine::<TestSpec>::new(TestState::Idle);
        assert_eq!(engine.valid_events(), vec![TestEvent::Start]);
    }

    #[test]
    fn test_final_state_has_no_events() {
        let engine = StatechartEngine::<TestSpec>::new(TestState::Done);
        assert!(engine.valid_events().is_empty());
        assert!(engine.is_in_final_state());
    }

    #[test]
    fn test_tracer_called() {
        use std::sync::{Arc, Mutex};

        #[derive(Clone)]
        struct RecordingTracer {
            transitions: Arc<Mutex<Vec<(String, String, String)>>>,
        }

        impl StatechartTracer for RecordingTracer {
            fn on_transition(&self, from: &str, event: &str, to: &str) {
                self.transitions.lock().unwrap().push((
                    from.to_string(),
                    event.to_string(),
                    to.to_string(),
                ));
            }
            fn on_invalid_transition(&self, _from: &str, _event: &str, _valid: &[String]) {}
        }

        let tracer = RecordingTracer {
            transitions: Arc::new(Mutex::new(Vec::new())),
        };
        let transitions = tracer.transitions.clone();

        let mut engine =
            StatechartEngine::<TestSpec>::with_tracer(TestState::Idle, Box::new(tracer));

        engine.send_event(TestEvent::Start).unwrap();
        engine.send_event(TestEvent::Finish).unwrap();

        let log = transitions.lock().unwrap();
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn test_snapshot_and_recent_transitions() {
        let mut engine = StatechartEngine::<TestSpec>::with_tracer(
            TestState::Idle,
            Box::new(LoggingTracer),
        );
        engine.send_event(TestEvent::Start).unwrap();
        engine.send_event(TestEvent::Finish).unwrap();

        let snapshot = engine.snapshot(Some("<scxml/>".to_string()));
        assert_eq!(snapshot.statechart_name, "test");
        assert_eq!(snapshot.current_state, "Done");
        assert_eq!(snapshot.scxml_hash, "test_hash");
        assert_eq!(snapshot.raw_scxml.as_deref(), Some("<scxml/>"));
        assert_eq!(snapshot.recent_transitions.len(), 2);
        assert_eq!(snapshot.recent_transitions[0].kind, TransitionKind::Valid);

        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("\"statechart_name\""));
        assert!(json.contains("\"recent_transitions\""));
    }
}
