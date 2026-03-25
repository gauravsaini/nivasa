//! Statechart runtime engine.
//!
//! The `StatechartEngine` is the **only** mechanism for state transitions at runtime.
//! There is no `set_state()` — all transitions go through `send_event()`.
//!
//! Invalid transitions are:
//! - **Debug builds**: panic with full diagnostic
//! - **Release builds**: return `Err(InvalidTransitionError)`

use std::fmt;
use std::marker::PhantomData;

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
    /// Phantom for spec type.
    _spec: PhantomData<S>,
}

impl<S: StatechartSpec> StatechartEngine<S> {
    /// Create a new engine in the given initial state.
    pub fn new(initial_state: S::State) -> Self {
        Self {
            current_state: initial_state,
            tracer: None,
            _spec: PhantomData,
        }
    }

    /// Create a new engine with a tracer.
    pub fn with_tracer(initial_state: S::State, tracer: Box<dyn StatechartTracer>) -> Self {
        Self {
            current_state: initial_state,
            tracer: Some(tracer),
            _spec: PhantomData,
        }
    }

    /// The **only** public method that changes state.
    ///
    /// Validates the (current_state, event) pair against the transition table.
    /// On success: transitions to the target state and returns it.
    /// On failure (debug): **panics** with full diagnostic info.
    /// On failure (release): returns `Err(InvalidTransitionError)`.
    pub fn send_event(
        &mut self,
        event: S::Event,
    ) -> Result<S::State, InvalidTransitionError<S>> {
        match S::transition(&self.current_state, &event) {
            Some(target) => {
                if let Some(ref tracer) = self.tracer {
                    tracer.on_transition(
                        &format!("{:?}", self.current_state),
                        &format!("{:?}", event),
                        &format!("{:?}", target),
                    );
                }
                self.current_state = target;
                Ok(target)
            }
            None => {
                let valid_events = S::valid_events_for(&self.current_state);

                if let Some(ref tracer) = self.tracer {
                    tracer.on_invalid_transition(
                        &format!("{:?}", self.current_state),
                        &format!("{:?}", event),
                        &valid_events
                            .iter()
                            .map(|e| format!("{:?}", e))
                            .collect::<Vec<_>>(),
                    );
                }

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
}
