//! # nivasa-statechart
//!
//! SCXML (W3C State Chart XML) engine for the Nivasa framework.
//!
//! This crate provides:
//! - **Parser**: Parse `.scxml` files into an in-memory state tree
//! - **Validator**: Check statecharts for completeness, reachability, determinism
//! - **Codegen**: Generate Rust enums, transition tables, and handler traits from SCXML
//! - **Engine**: Runtime statechart interpreter that enforces valid transitions
//!
//! ## Architecture
//!
//! The SCXML files in `statecharts/` are the **source of truth** for all state
//! machines in Nivasa. The `build.rs` script parses them and generates Rust code.
//! The `StatechartEngine` enforces transitions at runtime — there is no `set_state()`.
//!
//! ## SCXML Compliance
//!
//! Based on [W3C SCXML Recommendation (September 2015)](https://www.w3.org/TR/scxml/).

pub mod codegen;
pub mod engine;
pub mod parser;
pub mod types;
pub mod validator;

pub use engine::{
    InvalidTransitionError, LoggingTracer, StatechartEngine, StatechartSnapshot, StatechartSpec,
    StatechartTracer, TransitionKind, TransitionRecord,
};
pub use parser::ScxmlDocument;
pub use types::*;

// Include generated statechart code
// These are generated from .scxml files by build.rs
#[cfg(not(test))]
include!(concat!(env!("OUT_DIR"), "/mod.rs"));
