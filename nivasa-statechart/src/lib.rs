//! # nivasa-statechart
//!
//! SCXML (W3C State Chart XML) engine for the Nivasa framework.
//!
//! This crate provides:
//! - **Parser**: parse `.scxml` files into an in-memory state tree
//! - **Validator**: check statecharts for schema and semantic issues
//! - **Codegen**: generate Rust enums, transition tables, and handler traits from SCXML
//! - **Engine**: runtime statechart interpreter that enforces valid transitions
//!
//! ## How It Fits
//!
//! SCXML files in `statecharts/` are the source of truth for Nivasa state
//! machines. Build-time code generation produces Rust types from those files,
//! and `StatechartEngine` is the only runtime way to move between states.
//!
//! ## Start Here
//!
//! Parse an SCXML document, validate it, and generate Rust from the same source
//! of truth before you wire the generated state and event types into the
//! runtime engine.
//!
//! ```rust,no_run
//! use nivasa_statechart::{
//!     codegen::generate_rust,
//!     parser::ScxmlDocument,
//!     validate_scxml_schema,
//! };
//! use nivasa_statechart::validator::validate;
//!
//! let scxml = r#"
//! <scxml xmlns="http://www.w3.org/2005/07/scxml" initial="idle">
//!   <state id="idle" />
//! </scxml>
//! "#;
//!
//! let doc = ScxmlDocument::from_str(scxml).unwrap();
//! validate_scxml_schema("statecharts/example.scxml").unwrap();
//! assert!(validate(&doc).is_valid());
//! let generated = generate_rust(&doc);
//! assert!(generated.contains("enum ExampleState"));
//! ```
//!
//! When you already have generated state and event types, the runtime engine is
//! the only supported way to drive transitions.
//!
//! ## Runtime Example
//!
//! ```rust
//! use nivasa_statechart::{StatechartEngine, StatechartSpec};
//!
//! #[derive(Debug, Clone, Copy, PartialEq, Eq)]
//! enum DoorState {
//!     Closed,
//!     Open,
//! }
//!
//! #[derive(Debug, Clone, PartialEq, Eq)]
//! enum DoorEvent {
//!     Open,
//!     Close,
//! }
//!
//! struct DoorSpec;
//!
//! impl StatechartSpec for DoorSpec {
//!     type State = DoorState;
//!     type Event = DoorEvent;
//!
//!     fn transition(current: &Self::State, event: &Self::Event) -> Option<Self::State> {
//!         match (current, event) {
//!             (DoorState::Closed, DoorEvent::Open) => Some(DoorState::Open),
//!             (DoorState::Open, DoorEvent::Close) => Some(DoorState::Closed),
//!             _ => None,
//!         }
//!     }
//!
//!     fn valid_events_for(state: &Self::State) -> Vec<Self::Event> {
//!         match state {
//!             DoorState::Closed => vec![DoorEvent::Open],
//!             DoorState::Open => vec![DoorEvent::Close],
//!         }
//!     }
//!
//!     fn is_final(_: &Self::State) -> bool {
//!         false
//!     }
//!
//!     fn name() -> &'static str {
//!         "door"
//!     }
//!
//!     fn scxml_hash() -> &'static str {
//!         "demo"
//!     }
//! }
//!
//! let mut engine = StatechartEngine::<DoorSpec>::new(DoorState::Closed);
//! assert_eq!(engine.current_state(), DoorState::Closed);
//! assert_eq!(engine.send_event(DoorEvent::Open).unwrap(), DoorState::Open);
//! ```
//!
//! ## SCXML Compliance
//!
//! Based on [W3C SCXML Recommendation (September 2015)](https://www.w3.org/TR/scxml/).

pub mod codegen;
pub mod engine;
pub mod parser;
pub mod schema;
pub mod types;
pub mod validator;

pub use engine::{
    InvalidTransitionError, LoggingTracer, StatechartEngine, StatechartSnapshot, StatechartSpec,
    StatechartTracer, TransitionKind, TransitionRecord,
};
pub use parser::ScxmlDocument;
pub use schema::{
    scxml_schema_file, scxml_schema_root, validate_scxml_schema, SchemaDiagnostic,
    SchemaDiagnostics, SchemaValidationError,
};
pub use types::*;

// Include generated statechart code
// These are generated from .scxml files by build.rs
#[cfg(not(test))]
include!(concat!(env!("OUT_DIR"), "/mod.rs"));
