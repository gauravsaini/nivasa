//! Dependency injection building blocks.
//!
//! Start here:
//! - [`DependencyContainer`] owns registrations and resolution.
//! - [`ProviderRegistry`] stores provider entries by `TypeId`.
//! - [`ProviderScope`] selects singleton, scoped, or transient lifetimes.
//! - [`Lazy`] defers construction until first use.
//! - [`ValueProvider`], [`FactoryProvider`], and [`provider::ClassProvider`] are the main provider shapes.
//!
//! # Example
//! ```rust
//! use nivasa_core::di::{DependencyContainer, Lazy, ProviderRegistry, ProviderScope, ValueProvider};
//! use nivasa_core::di::provider::ClassProvider;
//! use nivasa_core::Provider;
//!
//! let container = DependencyContainer::new();
//! let registry = ProviderRegistry::new();
//! let value = ValueProvider::new(42_u32);
//! let lazy = Lazy::new(|| async { Ok::<_, nivasa_core::DiError>(7_u32) });
//!
//! assert!(registry.is_empty());
//! assert_eq!(value.metadata().scope, ProviderScope::Singleton);
//! # let _ = container;
//! # let _ = lazy;
//! ```
//!
//! # Provider shapes
//! ```rust
//! use nivasa_core::di::{ProviderScope, ValueProvider};
//! use nivasa_core::di::provider::{ClassProvider, FactoryProvider};
//! use nivasa_core::Provider;
//! use nivasa_core::DiError;
//!
//! let value = ValueProvider::new(String::from("hello"));
//! let factory = FactoryProvider::new(ProviderScope::Scoped, vec![], |_container| {
//!     Box::pin(async { Ok::<_, DiError>(String::from("built")) })
//! });
//! let class = ClassProvider::new(ProviderScope::Transient, vec![], |_container| {
//!     Box::pin(async { Ok::<_, DiError>(String::from("constructed")) })
//! });
//!
//! assert_eq!(value.metadata().scope, ProviderScope::Singleton);
//! assert_eq!(factory.metadata().scope, ProviderScope::Scoped);
//! assert_eq!(class.metadata().scope, ProviderScope::Transient);
//! ```

pub mod container;
pub mod error;
pub mod graph;
pub mod lazy;
pub mod lifecycle;
pub mod provider;
pub mod registry;

pub use graph::DependencyGraph;
pub use lazy::Lazy;
pub use registry::ProviderRegistry;

pub use container::DependencyContainer;
pub use error::DiError;
pub use provider::{FactoryProvider, Provider, ProviderMetadata, ProviderScope, ValueProvider};
