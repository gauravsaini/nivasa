pub mod dynamic;
pub mod lifecycle;
pub mod orchestrator;
pub mod registry;
pub mod runtime;

use crate::di::DependencyContainer;
use async_trait::async_trait;
use std::any::TypeId;

pub use dynamic::{ConfigurableModule, DynamicModule};
pub use orchestrator::{ModuleHookSet, ModuleOrchestrator, ModuleOrchestratorError};
pub use registry::{ModuleEntry, ModuleRegistry, ModuleRegistryError};
pub use runtime::{ModuleLifecycleError, ModuleRuntime};

/// Metadata for a Nivasa module.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ModuleMetadata {
    /// Module imports, identified by type.
    pub imports: Vec<TypeId>,
    /// Provider types owned by the module.
    pub providers: Vec<TypeId>,
    /// Controller types owned by the module.
    pub controllers: Vec<TypeId>,
    /// Provider types exported to other modules.
    pub exports: Vec<TypeId>,
    /// Middleware types applied by the module.
    pub middlewares: Vec<TypeId>,
    /// Marks the module as globally visible.
    pub is_global: bool,
}

impl ModuleMetadata {
    /// Create empty module metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set module imports.
    pub fn with_imports(mut self, imports: Vec<TypeId>) -> Self {
        self.imports = imports;
        self
    }

    /// Set module providers.
    pub fn with_providers(mut self, providers: Vec<TypeId>) -> Self {
        self.providers = providers;
        self
    }

    /// Set module controllers.
    pub fn with_controllers(mut self, controllers: Vec<TypeId>) -> Self {
        self.controllers = controllers;
        self
    }

    /// Set exported providers.
    pub fn with_exports(mut self, exports: Vec<TypeId>) -> Self {
        self.exports = exports;
        self
    }

    /// Set module middlewares.
    pub fn with_middlewares(mut self, middlewares: Vec<TypeId>) -> Self {
        self.middlewares = middlewares;
        self
    }

    /// Mark the module as global or local.
    pub fn with_global(mut self, is_global: bool) -> Self {
        self.is_global = is_global;
        self
    }
}

/// One route exposed by a controller listed on a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerRouteRegistration {
    /// HTTP method for the route.
    pub method: &'static str,
    /// Route path relative to the controller prefix.
    pub path: String,
    /// Handler method name.
    pub handler: &'static str,
}

impl ControllerRouteRegistration {
    /// Build a route registration entry.
    pub fn new(method: &'static str, path: impl Into<String>, handler: &'static str) -> Self {
        Self {
            method,
            path: path.into(),
            handler,
        }
    }
}

/// A controller plus the routes it contributes to a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleControllerRegistration {
    /// Controller type id.
    pub controller: TypeId,
    /// Routes exposed by the controller.
    pub routes: Vec<ControllerRouteRegistration>,
    /// Middleware types attached to the controller.
    pub middlewares: Vec<TypeId>,
}

impl ModuleControllerRegistration {
    /// Build a controller registration entry.
    pub fn new(
        controller: TypeId,
        routes: Vec<ControllerRouteRegistration>,
        middlewares: Vec<TypeId>,
    ) -> Self {
        Self {
            controller,
            routes,
            middlewares,
        }
    }
}

/// The core trait for all Nivasa modules.
#[async_trait]
pub trait Module: Send + Sync + 'static {
    /// Return module metadata used by the container and orchestrator.
    fn metadata(&self) -> ModuleMetadata;

    /// Register the module's providers into the container.
    async fn configure(
        &self,
        container: &DependencyContainer,
    ) -> Result<(), crate::di::error::DiError>;

    /// Return controller registrations for the module.
    fn controller_registrations(&self) -> Vec<ModuleControllerRegistration> {
        Vec::new()
    }
}

/// Called when a module is initialized.
#[async_trait]
pub trait OnModuleInit: Send + Sync {
    /// Run module initialization logic.
    async fn on_module_init(&self);
}

/// Called when a module is destroyed.
#[async_trait]
pub trait OnModuleDestroy: Send + Sync {
    /// Run module teardown logic.
    async fn on_module_destroy(&self);
}

/// Called when the application finishes bootstrapping.
#[async_trait]
pub trait OnApplicationBootstrap: Send + Sync {
    /// Run application bootstrap logic.
    async fn on_application_bootstrap(&self);
}

/// Called when the application is shutting down.
#[async_trait]
pub trait OnApplicationShutdown: Send + Sync {
    /// Run application shutdown logic.
    async fn on_application_shutdown(&self);
}
