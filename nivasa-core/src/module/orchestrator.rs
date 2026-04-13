use super::{
    Module, ModuleLifecycleError, ModuleRegistry, ModuleRegistryError, ModuleRuntime,
    OnApplicationBootstrap, OnApplicationShutdown, OnModuleDestroy, OnModuleInit,
};
use crate::di::DependencyContainer;
use async_trait::async_trait;
use std::any::TypeId;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use thiserror::Error;

type HookFuture<'a> = Pin<Box<dyn Future<Output = ()> + Send + 'a>>;
type ModuleHook<M> = for<'a> fn(&'a M) -> HookFuture<'a>;

#[derive(Clone, Copy)]
/// Hook set for module and application lifecycle callbacks.
///
/// Use `none()` for no hooks, `module_lifecycle()` for module init/destroy,
/// `application_lifecycle()` for app bootstrap/shutdown, or `all()` for both.
///
/// ```rust
/// use nivasa_core::module::ModuleHookSet;
///
/// let _hooks: ModuleHookSet<()> = ModuleHookSet::none();
/// ```
pub struct ModuleHookSet<M> {
    on_module_init: Option<ModuleHook<M>>,
    on_module_destroy: Option<ModuleHook<M>>,
    on_application_bootstrap: Option<ModuleHook<M>>,
    on_application_shutdown: Option<ModuleHook<M>>,
}

impl<M> ModuleHookSet<M> {
    /// No lifecycle hooks.
    pub fn none() -> Self {
        Self {
            on_module_init: None,
            on_module_destroy: None,
            on_application_bootstrap: None,
            on_application_shutdown: None,
        }
    }
}

impl<M> Default for ModuleHookSet<M> {
    fn default() -> Self {
        Self::none()
    }
}

impl<M> ModuleHookSet<M>
where
    M: OnModuleInit + OnModuleDestroy,
{
    /// Module init and destroy hooks only.
    pub fn module_lifecycle() -> Self {
        Self {
            on_module_init: Some(|module| Box::pin(module.on_module_init())),
            on_module_destroy: Some(|module| Box::pin(module.on_module_destroy())),
            ..Self::none()
        }
    }
}

impl<M> ModuleHookSet<M>
where
    M: OnApplicationBootstrap + OnApplicationShutdown,
{
    /// Application bootstrap and shutdown hooks only.
    pub fn application_lifecycle() -> Self {
        Self {
            on_application_bootstrap: Some(|module| Box::pin(module.on_application_bootstrap())),
            on_application_shutdown: Some(|module| Box::pin(module.on_application_shutdown())),
            ..Self::none()
        }
    }
}

impl<M> ModuleHookSet<M>
where
    M: OnModuleInit + OnModuleDestroy + OnApplicationBootstrap + OnApplicationShutdown,
{
    /// All module and application lifecycle hooks.
    pub fn all() -> Self {
        Self {
            on_module_init: Some(|module| Box::pin(module.on_module_init())),
            on_module_destroy: Some(|module| Box::pin(module.on_module_destroy())),
            on_application_bootstrap: Some(|module| Box::pin(module.on_application_bootstrap())),
            on_application_shutdown: Some(|module| Box::pin(module.on_application_shutdown())),
        }
    }
}

#[derive(Debug, Error)]
/// Errors from module orchestration.
///
/// ```rust,no_run
/// use nivasa_core::module::ModuleOrchestratorError;
///
/// # let error = ModuleOrchestratorError::MissingRuntime {
/// #     type_id: std::any::TypeId::of::<()>(),
/// # };
/// let _ = error.to_string();
/// ```
pub enum ModuleOrchestratorError {
    /// Registry layer failure.
    #[error(transparent)]
    Registry(#[from] ModuleRegistryError),
    /// Lifecycle engine failure.
    #[error(transparent)]
    Lifecycle(#[from] ModuleLifecycleError),
    /// Registered module has no runtime entry.
    #[error("module runtime missing for registered module {type_id:?}")]
    MissingRuntime { type_id: TypeId },
}

#[async_trait]
trait ManagedModuleRuntime: Send {
    fn state(&self) -> super::lifecycle::NivasaModuleState;
    fn container(&self) -> DependencyContainer;
    async fn load(&mut self) -> Result<(), ModuleLifecycleError>;
    async fn initialize_and_activate(&mut self) -> Result<(), ModuleLifecycleError>;
    async fn on_application_bootstrap(&self);
    async fn on_application_shutdown(&self);
    async fn destroy(&mut self) -> Result<(), ModuleLifecycleError>;
}

struct ManagedRuntime<M> {
    runtime: ModuleRuntime<M>,
    hooks: ModuleHookSet<M>,
}

impl<M> ManagedRuntime<M> {
    fn new(module: M, container: DependencyContainer, hooks: ModuleHookSet<M>) -> Self {
        Self {
            runtime: ModuleRuntime::with_container(module, container),
            hooks,
        }
    }
}

#[async_trait]
impl<M> ManagedModuleRuntime for ManagedRuntime<M>
where
    M: Module + Send + Sync + 'static,
{
    fn state(&self) -> super::lifecycle::NivasaModuleState {
        self.runtime.state()
    }

    fn container(&self) -> DependencyContainer {
        self.runtime.container().clone()
    }

    async fn load(&mut self) -> Result<(), ModuleLifecycleError> {
        self.runtime.load().await.map(|_| ())
    }

    async fn initialize_and_activate(&mut self) -> Result<(), ModuleLifecycleError> {
        self.runtime.initialize()?;
        if let Some(callback) = self.hooks.on_module_init {
            callback(self.runtime.module()).await;
        }
        self.runtime.activate()?;
        Ok(())
    }

    async fn on_application_bootstrap(&self) {
        if let Some(callback) = self.hooks.on_application_bootstrap {
            callback(self.runtime.module()).await;
        }
    }

    async fn on_application_shutdown(&self) {
        if let Some(callback) = self.hooks.on_application_shutdown {
            callback(self.runtime.module()).await;
        }
    }

    async fn destroy(&mut self) -> Result<(), ModuleLifecycleError> {
        self.runtime.begin_destroy()?;
        if let Some(callback) = self.hooks.on_module_destroy {
            callback(self.runtime.module()).await;
        }
        self.runtime.complete_destroy()?;
        Ok(())
    }
}

/// Coordinates module registration, bootstrap, and shutdown.
///
/// ```rust,no_run
/// use async_trait::async_trait;
/// use nivasa_core::di::{DependencyContainer, error::DiError};
/// use nivasa_core::module::{Module, ModuleMetadata, ModuleOrchestrator};
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
///
/// let mut orchestrator = ModuleOrchestrator::new();
/// let _registered = orchestrator.register(AppModule);
/// ```
pub struct ModuleOrchestrator {
    registry: ModuleRegistry,
    runtimes: HashMap<TypeId, Box<dyn ManagedModuleRuntime>>,
    activation_order: Vec<TypeId>,
}

impl Default for ModuleOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleOrchestrator {
    /// Create empty orchestrator.
    pub fn new() -> Self {
        Self {
            registry: ModuleRegistry::new(),
            runtimes: HashMap::new(),
            activation_order: Vec::new(),
        }
    }

    /// Register module with no extra hooks.
    ///
    /// ```rust,no_run
    /// use async_trait::async_trait;
    /// use nivasa_core::di::{DependencyContainer, error::DiError};
    /// use nivasa_core::module::{Module, ModuleMetadata, ModuleOrchestrator};
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
    ///
    /// let mut orchestrator = ModuleOrchestrator::new();
    /// let _ = orchestrator.register(AppModule);
    /// ```
    pub fn register<M>(&mut self, module: M) -> bool
    where
        M: Module + Send + Sync + 'static,
    {
        self.register_with_hooks(module, ModuleHookSet::none())
    }

    /// Register module with explicit lifecycle hooks.
    ///
    /// ```rust,no_run
    /// use async_trait::async_trait;
    /// use nivasa_core::di::{DependencyContainer, error::DiError};
    /// use nivasa_core::module::{Module, ModuleHookSet, ModuleMetadata, ModuleOrchestrator};
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
    ///
    /// let mut orchestrator = ModuleOrchestrator::new();
    /// let _ = orchestrator.register_with_hooks(AppModule, ModuleHookSet::none());
    /// ```
    pub fn register_with_hooks<M>(&mut self, module: M, hooks: ModuleHookSet<M>) -> bool
    where
        M: Module + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<M>();
        let inserted = self.registry.register(&module);
        let runtime = ManagedRuntime::new(module, DependencyContainer::new(), hooks);
        self.runtimes.insert(type_id, Box::new(runtime));
        inserted
    }

    /// Registry view for registered modules.
    ///
    /// ```rust,no_run
    /// use nivasa_core::module::ModuleOrchestrator;
    ///
    /// let orchestrator = ModuleOrchestrator::new();
    /// let _registry = orchestrator.registry();
    /// ```
    pub fn registry(&self) -> &ModuleRegistry {
        &self.registry
    }

    /// Activation order from last successful bootstrap.
    ///
    /// ```rust,no_run
    /// use nivasa_core::module::ModuleOrchestrator;
    ///
    /// let orchestrator = ModuleOrchestrator::new();
    /// assert!(orchestrator.activation_order().is_empty());
    /// ```
    pub fn activation_order(&self) -> &[TypeId] {
        &self.activation_order
    }

    /// Current module state by type.
    ///
    /// ```rust,no_run
    /// use async_trait::async_trait;
    /// use nivasa_core::di::{DependencyContainer, error::DiError};
    /// use nivasa_core::module::{Module, ModuleMetadata, ModuleOrchestrator};
    ///
    /// struct AppModule;
    ///
    /// #[async_trait]
    /// impl Module for AppModule {
    ///     fn metadata(&self) -> ModuleMetadata {
    ///         ModuleMetadata::new()
    ///     }
    ///
    ///     async fn configure(
    ///         &self,
    ///         _container: &nivasa_core::di::DependencyContainer,
    ///     ) -> Result<(), nivasa_core::di::error::DiError> {
    ///         Ok(())
    ///     }
    /// }
    ///
    /// let orchestrator = ModuleOrchestrator::new();
    /// let _state = orchestrator.state_for::<AppModule>();
    /// ```
    pub fn state_for<M: Module>(&self) -> Option<super::lifecycle::NivasaModuleState> {
        self.runtimes
            .get(&TypeId::of::<M>())
            .map(|runtime| runtime.state())
    }

    /// Bootstrap modules in dependency order, then run app bootstrap hooks.
    pub async fn bootstrap(&mut self) -> Result<&[TypeId], ModuleOrchestratorError> {
        let order = self.registry.resolve_order()?;

        for type_id in &order {
            self.seed_imported_exports(*type_id).await?;
            let runtime = self.runtime_mut(*type_id)?;
            runtime.load().await?;
            runtime.initialize_and_activate().await?;
        }

        for type_id in &order {
            let runtime = self.runtime_ref(*type_id)?;
            runtime.on_application_bootstrap().await;
        }

        self.activation_order = order;
        Ok(&self.activation_order)
    }

    /// Shut down modules in reverse activation order.
    pub async fn shutdown(&mut self) -> Result<(), ModuleOrchestratorError> {
        for type_id in self.activation_order.iter().rev().copied().collect::<Vec<_>>() {
            let runtime = self.runtime_ref(type_id)?;
            runtime.on_application_shutdown().await;
        }

        for type_id in self.activation_order.iter().rev().copied().collect::<Vec<_>>() {
            let runtime = self.runtime_mut(type_id)?;
            runtime.destroy().await?;
        }

        Ok(())
    }

    async fn seed_imported_exports(
        &self,
        type_id: TypeId,
    ) -> Result<(), ModuleOrchestratorError> {
        let target_container = self.runtime_ref(type_id)?.container();
        let import_sources = self.registry.import_sources_by_id(type_id)?;

        for source_type in import_sources {
            let source_container = self.runtime_ref(source_type)?.container();
            let exported_surface = self.registry.exported_surface_for(source_type)?;

            for exported in exported_surface {
                if let Some((metadata, value)) = source_container.export_singleton_value(exported).await
                {
                    target_container.import_singleton_value(metadata, value).await;
                }
            }
        }

        Ok(())
    }

    fn runtime_ref(
        &self,
        type_id: TypeId,
    ) -> Result<&(dyn ManagedModuleRuntime + '_), ModuleOrchestratorError> {
        match self.runtimes.get(&type_id) {
            Some(runtime) => Ok(runtime.as_ref()),
            None => Err(ModuleOrchestratorError::MissingRuntime { type_id }),
        }
    }

    fn runtime_mut(
        &mut self,
        type_id: TypeId,
    ) -> Result<&mut (dyn ManagedModuleRuntime + '_), ModuleOrchestratorError> {
        match self.runtimes.get_mut(&type_id) {
            Some(runtime) => Ok(runtime.as_mut()),
            None => Err(ModuleOrchestratorError::MissingRuntime { type_id }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModuleMetadata;
    use crate::module::lifecycle::NivasaModuleState;
    use std::sync::{Arc, Mutex};

    struct LeafModule {
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    struct SharedModule {
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    struct RootModule {
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    fn metadata(imports: Vec<TypeId>) -> ModuleMetadata {
        ModuleMetadata::new().with_imports(imports)
    }

    #[async_trait]
    impl Module for LeafModule {
        fn metadata(&self) -> ModuleMetadata {
            metadata(vec![])
        }

        async fn configure(
            &self,
            _container: &DependencyContainer,
        ) -> Result<(), crate::di::error::DiError> {
            self.events.lock().unwrap().push("leaf.configure");
            Ok(())
        }
    }

    #[async_trait]
    impl OnModuleInit for LeafModule {
        async fn on_module_init(&self) {
            self.events.lock().unwrap().push("leaf.init");
        }
    }

    #[async_trait]
    impl OnModuleDestroy for LeafModule {
        async fn on_module_destroy(&self) {
            self.events.lock().unwrap().push("leaf.destroy");
        }
    }

    #[async_trait]
    impl OnApplicationBootstrap for LeafModule {
        async fn on_application_bootstrap(&self) {
            self.events.lock().unwrap().push("leaf.bootstrap");
        }
    }

    #[async_trait]
    impl OnApplicationShutdown for LeafModule {
        async fn on_application_shutdown(&self) {
            self.events.lock().unwrap().push("leaf.shutdown");
        }
    }

    #[async_trait]
    impl Module for SharedModule {
        fn metadata(&self) -> ModuleMetadata {
            metadata(vec![TypeId::of::<LeafModule>()])
        }

        async fn configure(
            &self,
            _container: &DependencyContainer,
        ) -> Result<(), crate::di::error::DiError> {
            self.events.lock().unwrap().push("shared.configure");
            Ok(())
        }
    }

    #[async_trait]
    impl OnModuleInit for SharedModule {
        async fn on_module_init(&self) {
            self.events.lock().unwrap().push("shared.init");
        }
    }

    #[async_trait]
    impl OnModuleDestroy for SharedModule {
        async fn on_module_destroy(&self) {
            self.events.lock().unwrap().push("shared.destroy");
        }
    }

    #[async_trait]
    impl OnApplicationBootstrap for SharedModule {
        async fn on_application_bootstrap(&self) {
            self.events.lock().unwrap().push("shared.bootstrap");
        }
    }

    #[async_trait]
    impl OnApplicationShutdown for SharedModule {
        async fn on_application_shutdown(&self) {
            self.events.lock().unwrap().push("shared.shutdown");
        }
    }

    #[async_trait]
    impl Module for RootModule {
        fn metadata(&self) -> ModuleMetadata {
            metadata(vec![TypeId::of::<SharedModule>()])
        }

        async fn configure(
            &self,
            _container: &DependencyContainer,
        ) -> Result<(), crate::di::error::DiError> {
            self.events.lock().unwrap().push("root.configure");
            Ok(())
        }
    }

    #[async_trait]
    impl OnModuleInit for RootModule {
        async fn on_module_init(&self) {
            self.events.lock().unwrap().push("root.init");
        }
    }

    #[async_trait]
    impl OnModuleDestroy for RootModule {
        async fn on_module_destroy(&self) {
            self.events.lock().unwrap().push("root.destroy");
        }
    }

    #[async_trait]
    impl OnApplicationBootstrap for RootModule {
        async fn on_application_bootstrap(&self) {
            self.events.lock().unwrap().push("root.bootstrap");
        }
    }

    #[async_trait]
    impl OnApplicationShutdown for RootModule {
        async fn on_application_shutdown(&self) {
            self.events.lock().unwrap().push("root.shutdown");
        }
    }

    #[tokio::test]
    async fn bootstrap_uses_dependency_order_and_runs_app_hooks_after_activation() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut orchestrator = ModuleOrchestrator::new();
        orchestrator.register_with_hooks(
            RootModule {
                events: events.clone(),
            },
            ModuleHookSet::all(),
        );
        orchestrator.register_with_hooks(
            SharedModule {
                events: events.clone(),
            },
            ModuleHookSet::all(),
        );
        orchestrator.register_with_hooks(
            LeafModule {
                events: events.clone(),
            },
            ModuleHookSet::all(),
        );

        let order = orchestrator.bootstrap().await.unwrap();
        assert_eq!(
            order,
            &[
                TypeId::of::<LeafModule>(),
                TypeId::of::<SharedModule>(),
                TypeId::of::<RootModule>(),
            ],
        );
        assert_eq!(orchestrator.state_for::<LeafModule>(), Some(NivasaModuleState::Active));
        assert_eq!(orchestrator.state_for::<SharedModule>(), Some(NivasaModuleState::Active));
        assert_eq!(orchestrator.state_for::<RootModule>(), Some(NivasaModuleState::Active));

        assert_eq!(
            &*events.lock().unwrap(),
            &[
                "leaf.configure",
                "leaf.init",
                "shared.configure",
                "shared.init",
                "root.configure",
                "root.init",
                "leaf.bootstrap",
                "shared.bootstrap",
                "root.bootstrap",
            ],
        );
    }

    #[tokio::test]
    async fn shutdown_runs_application_hooks_and_destroys_in_reverse_order() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut orchestrator = ModuleOrchestrator::new();
        orchestrator.register_with_hooks(
            RootModule {
                events: events.clone(),
            },
            ModuleHookSet::all(),
        );
        orchestrator.register_with_hooks(
            SharedModule {
                events: events.clone(),
            },
            ModuleHookSet::all(),
        );
        orchestrator.register_with_hooks(
            LeafModule {
                events: events.clone(),
            },
            ModuleHookSet::all(),
        );

        orchestrator.bootstrap().await.unwrap();
        events.lock().unwrap().clear();

        orchestrator.shutdown().await.unwrap();

        assert_eq!(orchestrator.state_for::<LeafModule>(), Some(NivasaModuleState::Destroyed));
        assert_eq!(orchestrator.state_for::<SharedModule>(), Some(NivasaModuleState::Destroyed));
        assert_eq!(orchestrator.state_for::<RootModule>(), Some(NivasaModuleState::Destroyed));
        assert_eq!(
            &*events.lock().unwrap(),
            &[
                "root.shutdown",
                "shared.shutdown",
                "leaf.shutdown",
                "root.destroy",
                "shared.destroy",
                "leaf.destroy",
            ],
        );
    }
}
