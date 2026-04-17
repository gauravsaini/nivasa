pub mod dynamic;
pub mod event_emitter;
pub mod lifecycle;
pub mod orchestrator;
pub mod registry;
pub mod runtime;

use crate::di::DependencyContainer;
use async_trait::async_trait;
use std::any::TypeId;

pub use crate::lifecycle::{
    OnApplicationBootstrap, OnApplicationShutdown, OnModuleDestroy, OnModuleInit,
};
pub use dynamic::{ConfigurableModule, DynamicModule};
pub use event_emitter::{EventEmitter, EventEmitterModule};
pub use orchestrator::{ModuleHookSet, ModuleOrchestrator, ModuleOrchestratorError};
pub use registry::{ModuleEntry, ModuleRegistry, ModuleRegistryError};
pub use runtime::{ModuleLifecycleError, ModuleRuntime};

/// Metadata for a Nivasa module.
///
/// `ModuleMetadata` tells container which imports, providers, controllers,
/// exports, and middleware belong to one module.
///
/// ```rust
/// use std::any::TypeId;
/// use nivasa_core::module::ModuleMetadata;
///
/// struct AppService;
/// struct AppController;
///
/// let metadata = ModuleMetadata::new()
///     .with_providers(vec![TypeId::of::<AppService>()])
///     .with_controllers(vec![TypeId::of::<AppController>()])
///     .with_global(true);
///
/// assert!(metadata.is_global);
/// assert_eq!(metadata.providers, vec![TypeId::of::<AppService>()]);
/// assert_eq!(metadata.controllers, vec![TypeId::of::<AppController>()]);
/// ```
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
///
/// ```rust
/// use nivasa_core::module::ControllerRouteRegistration;
///
/// let route = ControllerRouteRegistration::new("GET", "/health", "health");
///
/// assert_eq!(route.method, "GET");
/// assert_eq!(route.path, "/health");
/// assert_eq!(route.handler, "health");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerRouteRegistration {
    /// HTTP method for the route.
    pub method: &'static str,
    /// Route path relative to the controller prefix.
    pub path: String,
    /// Handler method name.
    pub handler: &'static str,
    /// Optional throttle window attached to the route.
    pub throttle: Option<RouteThrottleRegistration>,
    /// Skip throttling entirely for the route.
    pub skip_throttle: bool,
}

impl ControllerRouteRegistration {
    /// Build a route registration entry.
    pub fn new(method: &'static str, path: impl Into<String>, handler: &'static str) -> Self {
        Self {
            method,
            path: path.into(),
            handler,
            throttle: None,
            skip_throttle: false,
        }
    }

    /// Attach a throttle window to the route.
    pub fn with_throttle(mut self, limit: u32, ttl_secs: u64) -> Self {
        self.throttle = Some(RouteThrottleRegistration::new(limit, ttl_secs));
        self
    }

    /// Mark the route as exempt from throttling.
    pub fn skip_throttle(mut self) -> Self {
        self.skip_throttle = true;
        self.throttle = None;
        self
    }
}

/// Throttle metadata attached to a route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteThrottleRegistration {
    /// Number of requests allowed per window.
    pub limit: u32,
    /// Window duration in seconds.
    pub ttl_secs: u64,
}

impl RouteThrottleRegistration {
    /// Build a route throttle window.
    pub fn new(limit: u32, ttl_secs: u64) -> Self {
        Self { limit, ttl_secs }
    }
}

/// A controller plus the routes it contributes to a module.
///
/// ```rust
/// use std::any::TypeId;
/// use nivasa_core::module::{
///     ControllerRouteRegistration,
///     ModuleControllerRegistration,
/// };
///
/// struct AppController;
///
/// let routes = vec![ControllerRouteRegistration::new("GET", "/health", "health")];
/// let registration = ModuleControllerRegistration::new(TypeId::of::<AppController>(), routes, vec![]);
///
/// assert_eq!(registration.routes.len(), 1);
/// ```
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
///
/// ```rust,no_run
/// use async_trait::async_trait;
/// use nivasa_core::di::{DependencyContainer, error::DiError};
/// use nivasa_core::module::{Module, ModuleMetadata};
///
/// struct AppModule;
///
/// #[async_trait]
/// impl Module for AppModule {
///     fn metadata(&self) -> ModuleMetadata {
///         ModuleMetadata::new()
///     }
///
///     async fn configure(&self, _container: &DependencyContainer) -> Result<(), DiError> {
///         Ok(())
///     }
/// }
/// ```
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
