//! Lifecycle hook traits.
//!
//! These traits correspond to SCXML `<onentry>` and `<onexit>` actions
//! in the module lifecycle statechart.

use async_trait::async_trait;

/// Called after the module's providers are registered and dependencies resolved.
/// Maps to `<onentry>` of the `Initialized` state in `nivasa.module.scxml`.
#[async_trait]
pub trait OnModuleInit: Send + Sync {
    async fn on_module_init(&self);
}

/// Called when the module is being destroyed during shutdown.
/// Maps to `<onentry>` of the `Destroying` state in `nivasa.module.scxml`.
#[async_trait]
pub trait OnModuleDestroy: Send + Sync {
    async fn on_module_destroy(&self);
}

/// Called after ALL modules have been initialized.
/// Maps to `<onentry>` of the `Running` → `Listening` state in `nivasa.application.scxml`.
#[async_trait]
pub trait OnApplicationBootstrap: Send + Sync {
    async fn on_application_bootstrap(&self);
}

/// Called when the application receives a shutdown signal.
/// Maps to `<onentry>` of the `Draining` state in `nivasa.application.scxml`.
#[async_trait]
pub trait OnApplicationShutdown: Send + Sync {
    async fn on_application_shutdown(&self, signal: Option<&str>);
}
