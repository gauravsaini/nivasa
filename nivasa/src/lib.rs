//! # Nivasa
//!
//! A modular, SCXML-driven Rust web framework with NestJS pattern compliance.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use nivasa::prelude::*;
//! ```
//!
//! ## Architecture
//!
//! Every lifecycle in Nivasa is modeled as a W3C SCXML statechart.
//! State transitions are code-generated from `.scxml` files and enforced
//! at compile time and runtime.

pub mod application;

/// The prelude — import everything you need with `use nivasa::prelude::*`.
pub mod prelude {
    pub use crate::application::{
        ServerOptions, ServerOptionsBuilder, VersioningOptions, VersioningOptionsBuilder,
        VersioningStrategy,
    };
    pub use nivasa_common::HttpException;
    pub use nivasa_core::DependencyContainer;
    pub use nivasa_statechart::{StatechartEngine, StatechartSpec};
}

pub use application::{
    ServerOptions, ServerOptionsBuilder, VersioningOptions, VersioningOptionsBuilder,
    VersioningStrategy,
};
pub use nivasa_common;
pub use nivasa_core;
pub use nivasa_macros;
pub use nivasa_statechart;
