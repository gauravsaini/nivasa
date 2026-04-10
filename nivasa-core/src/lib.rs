//! # nivasa-core
//!
//! Core foundation of the Nivasa framework.
//!
//! Start here:
//! - `di` for dependency injection and providers
//! - `module` for module metadata, registration, and lifecycle hooks
//! - `reflector` for metadata lookup helpers
//!
//! Common imports:
//!
//! ```rust
//! use nivasa_core::{DependencyContainer, ModuleMetadata, ModuleRegistry};
//!
//! let _container = DependencyContainer::new();
//! let _registry = ModuleRegistry::new();
//! let _metadata = ModuleMetadata::default();
//! ```
//!
//! ```rust
//! use nivasa_core::{
//!     OnApplicationBootstrap, OnApplicationShutdown, OnModuleDestroy, OnModuleInit,
//! };
//!
//! fn use_lifecycle_traits() {}
//! ```
//!
//! The crate re-exports the most common entry points so users can import from
//! `nivasa_core` directly when they do not need crate-internal modules.

pub mod di;
pub mod lifecycle;
pub mod module;
pub mod reflector;

// Re-exports
pub use di::{DependencyContainer, DiError, Provider, ProviderScope};
pub use module::{
    Module, ModuleEntry, ModuleMetadata, ModuleRegistry, ModuleRegistryError,
    OnApplicationBootstrap, OnApplicationShutdown, OnModuleDestroy, OnModuleInit,
};
pub use reflector::Reflector;
