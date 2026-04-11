//! Lifecycle hook traits.
//!
//! These traits correspond to SCXML `<onentry>` and `<onexit>` actions
//! in the module lifecycle statechart.
//!
//! # Examples
//!
//! ```rust,no_run
//! use async_trait::async_trait;
//! use nivasa_core::lifecycle::{
//!     OnApplicationBootstrap, OnApplicationShutdown, OnModuleDestroy, OnModuleInit,
//! };
//!
//! struct AppHooks;
//!
//! #[async_trait]
//! impl OnModuleInit for AppHooks {
//!     async fn on_module_init(&self) {}
//! }
//!
//! #[async_trait]
//! impl OnModuleDestroy for AppHooks {
//!     async fn on_module_destroy(&self) {}
//! }
//!
//! #[async_trait]
//! impl OnApplicationBootstrap for AppHooks {
//!     async fn on_application_bootstrap(&self) {}
//! }
//!
//! #[async_trait]
//! impl OnApplicationShutdown for AppHooks {
//!     async fn on_application_shutdown(&self, _signal: Option<&str>) {}
//! }
//! ```

use async_trait::async_trait;

/// Called after the module's providers are registered and dependencies resolved.
/// Maps to `<onentry>` of the `Initialized` state in `nivasa.module.scxml`.
///
/// ```rust,no_run
/// use async_trait::async_trait;
/// use nivasa_core::lifecycle::OnModuleInit;
///
/// struct AppModule;
///
/// #[async_trait]
/// impl OnModuleInit for AppModule {
///     async fn on_module_init(&self) {}
/// }
/// ```
#[async_trait]
pub trait OnModuleInit: Send + Sync {
    async fn on_module_init(&self);
}

/// Called when the module is being destroyed during shutdown.
/// Maps to `<onentry>` of the `Destroying` state in `nivasa.module.scxml`.
///
/// ```rust,no_run
/// use async_trait::async_trait;
/// use nivasa_core::lifecycle::OnModuleDestroy;
///
/// struct AppModule;
///
/// #[async_trait]
/// impl OnModuleDestroy for AppModule {
///     async fn on_module_destroy(&self) {}
/// }
/// ```
#[async_trait]
pub trait OnModuleDestroy: Send + Sync {
    async fn on_module_destroy(&self);
}

/// Called after ALL modules have been initialized.
/// Maps to `<onentry>` of the `Running` → `Listening` state in `nivasa.application.scxml`.
///
/// ```rust,no_run
/// use async_trait::async_trait;
/// use nivasa_core::lifecycle::OnApplicationBootstrap;
///
/// struct AppHooks;
///
/// #[async_trait]
/// impl OnApplicationBootstrap for AppHooks {
///     async fn on_application_bootstrap(&self) {}
/// }
/// ```
#[async_trait]
pub trait OnApplicationBootstrap: Send + Sync {
    async fn on_application_bootstrap(&self);
}

/// Called when the application receives a shutdown signal.
/// Maps to `<onentry>` of the `Draining` state in `nivasa.application.scxml`.
///
/// ```rust,no_run
/// use async_trait::async_trait;
/// use nivasa_core::lifecycle::OnApplicationShutdown;
///
/// struct AppHooks;
///
/// #[async_trait]
/// impl OnApplicationShutdown for AppHooks {
///     async fn on_application_shutdown(&self, signal: Option<&str>) {
///         let _ = signal;
///     }
/// }
/// ```
#[async_trait]
pub trait OnApplicationShutdown: Send + Sync {
    async fn on_application_shutdown(&self, signal: Option<&str>);
}
