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
    pub imports: Vec<TypeId>,
    pub providers: Vec<TypeId>,
    pub controllers: Vec<TypeId>,
    pub exports: Vec<TypeId>,
    pub is_global: bool,
}

impl ModuleMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_imports(mut self, imports: Vec<TypeId>) -> Self {
        self.imports = imports;
        self
    }

    pub fn with_providers(mut self, providers: Vec<TypeId>) -> Self {
        self.providers = providers;
        self
    }

    pub fn with_controllers(mut self, controllers: Vec<TypeId>) -> Self {
        self.controllers = controllers;
        self
    }

    pub fn with_exports(mut self, exports: Vec<TypeId>) -> Self {
        self.exports = exports;
        self
    }

    pub fn with_global(mut self, is_global: bool) -> Self {
        self.is_global = is_global;
        self
    }
}

/// One route exposed by a controller listed on a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerRouteRegistration {
    pub method: &'static str,
    pub path: String,
    pub handler: &'static str,
}

impl ControllerRouteRegistration {
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
    pub controller: TypeId,
    pub routes: Vec<ControllerRouteRegistration>,
}

impl ModuleControllerRegistration {
    pub fn new(controller: TypeId, routes: Vec<ControllerRouteRegistration>) -> Self {
        Self { controller, routes }
    }
}

/// The core trait for all Nivasa modules.
#[async_trait]
pub trait Module: Send + Sync + 'static {
    fn metadata(&self) -> ModuleMetadata;
    async fn configure(
        &self,
        container: &DependencyContainer,
    ) -> Result<(), crate::di::error::DiError>;

    fn controller_registrations(&self) -> Vec<ModuleControllerRegistration> {
        Vec::new()
    }
}

/// Lifecycle hook traits
#[async_trait]
pub trait OnModuleInit: Send + Sync {
    async fn on_module_init(&self);
}
#[async_trait]
pub trait OnModuleDestroy: Send + Sync {
    async fn on_module_destroy(&self);
}
#[async_trait]
pub trait OnApplicationBootstrap: Send + Sync {
    async fn on_application_bootstrap(&self);
}
#[async_trait]
pub trait OnApplicationShutdown: Send + Sync {
    async fn on_application_shutdown(&self);
}
