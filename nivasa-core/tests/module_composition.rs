use async_trait::async_trait;
use nivasa_core::di::error::DiError;
use nivasa_core::module::{
    Module, ModuleMetadata, ModuleOrchestrator, ModuleRegistry, ModuleRegistryError,
};
use std::any::{type_name, TypeId};
use std::sync::{Arc, Mutex};

struct PublicService;
struct HiddenService;
struct GlobalService;

struct SimpleModule;
struct LeafModule;
struct ReExportingModule;
struct ConsumerModule;
struct GlobalModule;
struct BrokenExportModule;
struct ImportedPublicService;
struct ImportedHiddenService;
struct ExportingSingletonModule;
struct ImportingSingletonModule {
    seen_public: Arc<Mutex<bool>>,
    saw_hidden_error: Arc<Mutex<bool>>,
}
#[derive(Clone)]
struct LifecycleProbeModule {
    events: Arc<Mutex<Vec<&'static str>>>,
}

fn metadata(
    imports: Vec<TypeId>,
    providers: Vec<TypeId>,
    exports: Vec<TypeId>,
    is_global: bool,
) -> ModuleMetadata {
    ModuleMetadata::new()
        .with_imports(imports)
        .with_providers(providers)
        .with_exports(exports)
        .with_global(is_global)
}

#[async_trait]
impl Module for LeafModule {
    fn metadata(&self) -> ModuleMetadata {
        metadata(
            vec![],
            vec![TypeId::of::<PublicService>(), TypeId::of::<HiddenService>()],
            vec![TypeId::of::<PublicService>()],
            false,
        )
    }

    async fn configure(
        &self,
        _container: &nivasa_core::di::DependencyContainer,
    ) -> Result<(), nivasa_core::di::error::DiError> {
        Ok(())
    }
}

#[async_trait]
impl Module for SimpleModule {
    fn metadata(&self) -> ModuleMetadata {
        metadata(
            vec![],
            vec![TypeId::of::<PublicService>()],
            vec![TypeId::of::<PublicService>()],
            false,
        )
    }

    async fn configure(
        &self,
        _container: &nivasa_core::di::DependencyContainer,
    ) -> Result<(), nivasa_core::di::error::DiError> {
        Ok(())
    }
}

#[async_trait]
impl Module for ReExportingModule {
    fn metadata(&self) -> ModuleMetadata {
        metadata(
            vec![TypeId::of::<LeafModule>()],
            vec![],
            vec![TypeId::of::<PublicService>()],
            false,
        )
    }

    async fn configure(
        &self,
        _container: &nivasa_core::di::DependencyContainer,
    ) -> Result<(), nivasa_core::di::error::DiError> {
        Ok(())
    }
}

#[async_trait]
impl Module for ConsumerModule {
    fn metadata(&self) -> ModuleMetadata {
        metadata(
            vec![TypeId::of::<ReExportingModule>()],
            vec![],
            vec![],
            false,
        )
    }

    async fn configure(
        &self,
        _container: &nivasa_core::di::DependencyContainer,
    ) -> Result<(), nivasa_core::di::error::DiError> {
        Ok(())
    }
}

#[async_trait]
impl Module for GlobalModule {
    fn metadata(&self) -> ModuleMetadata {
        metadata(
            vec![],
            vec![TypeId::of::<GlobalService>()],
            vec![TypeId::of::<GlobalService>()],
            true,
        )
    }

    async fn configure(
        &self,
        _container: &nivasa_core::di::DependencyContainer,
    ) -> Result<(), nivasa_core::di::error::DiError> {
        Ok(())
    }
}

#[async_trait]
impl Module for BrokenExportModule {
    fn metadata(&self) -> ModuleMetadata {
        metadata(
            vec![],
            vec![TypeId::of::<HiddenService>()],
            vec![TypeId::of::<PublicService>()],
            false,
        )
    }

    async fn configure(
        &self,
        _container: &nivasa_core::di::DependencyContainer,
    ) -> Result<(), nivasa_core::di::error::DiError> {
        Ok(())
    }
}

#[async_trait]
impl Module for ExportingSingletonModule {
    fn metadata(&self) -> ModuleMetadata {
        metadata(
            vec![],
            vec![
                TypeId::of::<ImportedPublicService>(),
                TypeId::of::<ImportedHiddenService>(),
            ],
            vec![TypeId::of::<ImportedPublicService>()],
            false,
        )
    }

    async fn configure(
        &self,
        container: &nivasa_core::di::DependencyContainer,
    ) -> Result<(), nivasa_core::di::error::DiError> {
        container.register_value(ImportedPublicService).await;
        container.register_value(ImportedHiddenService).await;
        Ok(())
    }
}

#[async_trait]
impl Module for ImportingSingletonModule {
    fn metadata(&self) -> ModuleMetadata {
        metadata(
            vec![TypeId::of::<ExportingSingletonModule>()],
            vec![],
            vec![],
            false,
        )
    }

    async fn configure(
        &self,
        container: &nivasa_core::di::DependencyContainer,
    ) -> Result<(), nivasa_core::di::error::DiError> {
        let public = container.resolve::<ImportedPublicService>().await;
        *self.seen_public.lock().unwrap() = public.is_ok();

        let hidden = container.resolve::<ImportedHiddenService>().await;
        *self.saw_hidden_error.lock().unwrap() =
            matches!(hidden, Err(DiError::ProviderNotFound(_)));

        Ok(())
    }
}

#[async_trait]
impl Module for LifecycleProbeModule {
    fn metadata(&self) -> ModuleMetadata {
        metadata(vec![], vec![], vec![], false)
    }

    async fn configure(
        &self,
        _container: &nivasa_core::di::DependencyContainer,
    ) -> Result<(), nivasa_core::di::error::DiError> {
        self.events.lock().unwrap().push("probe.configure");
        Ok(())
    }
}

#[async_trait]
impl nivasa_core::module::OnModuleInit for LifecycleProbeModule {
    async fn on_module_init(&self) {
        self.events.lock().unwrap().push("probe.init");
    }
}

#[async_trait]
impl nivasa_core::module::OnModuleDestroy for LifecycleProbeModule {
    async fn on_module_destroy(&self) {
        self.events.lock().unwrap().push("probe.destroy");
    }
}

#[async_trait]
impl nivasa_core::module::OnApplicationBootstrap for LifecycleProbeModule {
    async fn on_application_bootstrap(&self) {
        self.events.lock().unwrap().push("probe.bootstrap");
    }
}

#[async_trait]
impl nivasa_core::module::OnApplicationShutdown for LifecycleProbeModule {
    async fn on_application_shutdown(&self) {
        self.events.lock().unwrap().push("probe.shutdown");
    }
}

#[test]
fn import_resolution_and_re_exports_respect_export_boundaries() {
    let mut registry = ModuleRegistry::new();
    registry.register(&SimpleModule);
    registry.register(&LeafModule);
    registry.register(&ReExportingModule);
    registry.register(&ConsumerModule);

    let simple_visible = registry.visible_exports::<SimpleModule>().unwrap();
    assert!(simple_visible.contains(&TypeId::of::<PublicService>()));

    let leaf_visible = registry.visible_exports::<LeafModule>().unwrap();
    assert!(leaf_visible.contains(&TypeId::of::<PublicService>()));
    assert!(!leaf_visible.contains(&TypeId::of::<HiddenService>()));

    let re_exporting_visible = registry.visible_exports::<ReExportingModule>().unwrap();
    assert!(re_exporting_visible.contains(&TypeId::of::<PublicService>()));
    assert!(!re_exporting_visible.contains(&TypeId::of::<HiddenService>()));

    let consumer_visible = registry.visible_exports::<ConsumerModule>().unwrap();
    assert!(consumer_visible.contains(&TypeId::of::<PublicService>()));
    assert!(!consumer_visible.contains(&TypeId::of::<HiddenService>()));
}

#[test]
fn global_modules_are_visible_without_explicit_import() {
    let mut registry = ModuleRegistry::new();
    registry.register(&LeafModule);
    registry.register(&ReExportingModule);
    registry.register(&GlobalModule);
    registry.register(&ConsumerModule);

    let visible = registry.visible_exports::<ConsumerModule>().unwrap();
    assert!(visible.contains(&TypeId::of::<GlobalService>()));
    assert!(registry
        .is_visible_to::<ConsumerModule, GlobalService>()
        .unwrap());
}

#[test]
fn invalid_exports_are_rejected_during_visibility_resolution() {
    let mut registry = ModuleRegistry::new();
    registry.register(&BrokenExportModule);

    let err = registry
        .visible_exports::<BrokenExportModule>()
        .unwrap_err();
    match err {
        ModuleRegistryError::InvalidExport { module, exported } => {
            assert_eq!(module, type_name::<BrokenExportModule>());
            assert_eq!(exported, TypeId::of::<PublicService>());
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn orchestrator_seeds_imported_exported_singletons_without_hidden_providers() {
    let seen_public = Arc::new(Mutex::new(false));
    let saw_hidden_error = Arc::new(Mutex::new(false));
    let mut orchestrator = ModuleOrchestrator::new();

    orchestrator.register(ExportingSingletonModule);
    orchestrator.register(ImportingSingletonModule {
        seen_public: seen_public.clone(),
        saw_hidden_error: saw_hidden_error.clone(),
    });

    orchestrator.bootstrap().await.unwrap();

    assert!(*seen_public.lock().unwrap());
    assert!(*saw_hidden_error.lock().unwrap());
}

#[tokio::test]
async fn orchestrator_hook_sets_only_fire_their_selected_callbacks() {
    async fn run_with_hooks(
        hooks: nivasa_core::module::ModuleHookSet<LifecycleProbeModule>,
    ) -> Vec<&'static str> {
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut orchestrator = ModuleOrchestrator::new();
        orchestrator.register_with_hooks(
            LifecycleProbeModule {
                events: Arc::clone(&events),
            },
            hooks,
        );

        orchestrator.bootstrap().await.unwrap();
        orchestrator.shutdown().await.unwrap();

        let snapshot = events.lock().unwrap().clone();
        snapshot
    }

    let none = run_with_hooks(nivasa_core::module::ModuleHookSet::none()).await;
    assert_eq!(none, &["probe.configure"]);

    let module_lifecycle =
        run_with_hooks(nivasa_core::module::ModuleHookSet::module_lifecycle()).await;
    assert_eq!(module_lifecycle, &["probe.configure", "probe.init", "probe.destroy"]);

    let application_lifecycle =
        run_with_hooks(nivasa_core::module::ModuleHookSet::application_lifecycle()).await;
    assert_eq!(
        application_lifecycle,
        &["probe.configure", "probe.bootstrap", "probe.shutdown"]
    );

    let all = run_with_hooks(nivasa_core::module::ModuleHookSet::all()).await;
    assert_eq!(
        all,
        &[
            "probe.configure",
            "probe.init",
            "probe.bootstrap",
            "probe.shutdown",
            "probe.destroy",
        ]
    );
}
