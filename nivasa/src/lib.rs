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
        AppBootstrapConfig, ServerOptions, ServerOptionsBuilder, VersioningOptions,
        VersioningOptionsBuilder, VersioningStrategy,
    };
    pub use nivasa_common::{HttpException, HttpStatus};
    pub use nivasa_core::di::Lazy;
    pub use nivasa_core::di::provider::Injectable;
    pub use nivasa_core::{
        DependencyContainer, DiError, Module, ModuleMetadata, OnApplicationBootstrap,
        OnApplicationShutdown, OnModuleDestroy, OnModuleInit, Provider, ProviderScope,
    };
    pub use nivasa_macros::{injectable, module, scxml_handler};
    pub use nivasa_statechart::{StatechartEngine, StatechartSpec};
}

pub use application::{
    AppBootstrapConfig, ServerOptions, ServerOptionsBuilder, VersioningOptions,
    VersioningOptionsBuilder, VersioningStrategy,
};
pub use nivasa_common::{self, HttpException, HttpStatus};
pub use nivasa_core::di::Lazy;
pub use nivasa_core::di::provider::Injectable;
pub use nivasa_core::{
    self, DependencyContainer, DiError, Module, ModuleEntry, ModuleMetadata, ModuleRegistry,
    ModuleRegistryError, OnApplicationBootstrap, OnApplicationShutdown, OnModuleDestroy,
    OnModuleInit, Provider, ProviderScope,
};
pub use nivasa_macros::{self, injectable, module, scxml_handler};
pub use nivasa_statechart::{self, StatechartEngine, StatechartSpec};
