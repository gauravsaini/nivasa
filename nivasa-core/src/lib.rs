//! # nivasa-core
//!
//! Core foundation of the Nivasa framework.
//!
//! Provides:
//! - **DI Container**: Dependency injection with singleton, scoped, and transient providers
//! - **Module System**: NestJS-compatible module composition with imports/exports
//! - **Application Lifecycle**: SCXML-driven lifecycle management
//! - **Provider Traits**: `Injectable`, `Module`, lifecycle hooks

pub mod di;
pub mod lifecycle;
pub mod module;

// Re-exports
pub use di::{DependencyContainer, Provider, ProviderScope, DiError};
