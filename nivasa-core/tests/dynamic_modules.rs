use async_trait::async_trait;
use nivasa_core::module::{
    ConfigurableModule, DynamicModule, Module, ModuleMetadata, ModuleOrchestrator, ModuleRegistry,
};
use std::any::TypeId;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

struct RootService;
struct FeatureService;
struct FeatureServiceTwo;
struct ConsumerService;
struct ReExportedService;
struct RootDynamicModuleMarker;
struct FeatureDynamicModuleMarker;
struct FeatureDynamicModuleMarkerTwo;
struct DynamicReExportModuleMarker;
struct InvalidDynamicModuleMarker;
struct DynamicConsumerModule;
struct DynamicImportingConsumerModule;
struct DynamicImportingConsumerModuleTwo;
struct DynamicReExportConsumerModule;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DynamicOptions {
    provider: TypeId,
    is_global: bool,
}

struct ExampleDynamicModule;

impl ConfigurableModule for ExampleDynamicModule {
    type Options = DynamicOptions;

    fn for_root(options: Self::Options) -> DynamicModule {
        DynamicModule::new(
            ModuleMetadata::new()
                .with_global(options.is_global)
                .with_exports(vec![options.provider]),
        )
        .with_providers(vec![options.provider])
    }

    fn for_feature(options: Self::Options) -> DynamicModule {
        DynamicModule::new(ModuleMetadata::new().with_exports(vec![options.provider]))
            .with_providers(vec![options.provider])
    }
}

#[async_trait]
impl Module for DynamicConsumerModule {
    fn metadata(&self) -> ModuleMetadata {
        ModuleMetadata::new().with_providers(vec![TypeId::of::<ConsumerService>()])
    }

    async fn configure(
        &self,
        _container: &nivasa_core::di::DependencyContainer,
    ) -> Result<(), nivasa_core::di::error::DiError> {
        Ok(())
    }
}

#[async_trait]
impl Module for DynamicImportingConsumerModule {
    fn metadata(&self) -> ModuleMetadata {
        ModuleMetadata::new()
            .with_imports(vec![TypeId::of::<FeatureDynamicModuleMarker>()])
            .with_providers(vec![TypeId::of::<ConsumerService>()])
    }

    async fn configure(
        &self,
        _container: &nivasa_core::di::DependencyContainer,
    ) -> Result<(), nivasa_core::di::error::DiError> {
        Ok(())
    }
}

#[async_trait]
impl Module for DynamicImportingConsumerModuleTwo {
    fn metadata(&self) -> ModuleMetadata {
        ModuleMetadata::new()
            .with_imports(vec![TypeId::of::<FeatureDynamicModuleMarkerTwo>()])
            .with_providers(vec![TypeId::of::<ConsumerService>()])
    }

    async fn configure(
        &self,
        _container: &nivasa_core::di::DependencyContainer,
    ) -> Result<(), nivasa_core::di::error::DiError> {
        Ok(())
    }
}

#[async_trait]
impl Module for DynamicReExportConsumerModule {
    fn metadata(&self) -> ModuleMetadata {
        ModuleMetadata::new()
            .with_imports(vec![TypeId::of::<DynamicReExportModuleMarker>()])
            .with_providers(vec![TypeId::of::<ConsumerService>()])
    }

    async fn configure(
        &self,
        _container: &nivasa_core::di::DependencyContainer,
    ) -> Result<(), nivasa_core::di::error::DiError> {
        Ok(())
    }
}

#[test]
fn dynamic_module_tracks_metadata_and_extra_providers() {
    let module = DynamicModule::new(
        ModuleMetadata::new()
            .with_exports(vec![TypeId::of::<RootService>()])
            .with_global(true),
    )
    .with_providers(vec![TypeId::of::<RootService>()]);

    assert!(module.metadata.is_global);
    assert_eq!(module.metadata.exports, vec![TypeId::of::<RootService>()]);
    assert_eq!(module.providers, vec![TypeId::of::<RootService>()]);
}

#[test]
fn dynamic_module_debug_clone_and_equality_track_pre_bootstrap_presence() {
    let base = DynamicModule::new(ModuleMetadata::new())
        .with_providers(vec![TypeId::of::<RootService>()])
        .with_global(true);
    let with_hook = base.clone().with_pre_bootstrap(|| Ok(()));
    let cloned = with_hook.clone();

    assert_eq!(with_hook, cloned);
    assert_ne!(base, with_hook);
    assert!(format!("{with_hook:?}").contains("has_pre_bootstrap: true"));
}

#[test]
fn configurable_modules_can_build_root_and_feature_variants() {
    let root = ExampleDynamicModule::for_root(DynamicOptions {
        provider: TypeId::of::<RootService>(),
        is_global: true,
    });
    let feature = ExampleDynamicModule::for_feature(DynamicOptions {
        provider: TypeId::of::<FeatureService>(),
        is_global: true,
    });

    assert!(root.metadata.is_global);
    assert_eq!(root.providers, vec![TypeId::of::<RootService>()]);

    assert!(!feature.metadata.is_global);
    assert_eq!(feature.providers, vec![TypeId::of::<FeatureService>()]);
}

#[test]
fn dynamic_module_pre_bootstrap_callback_runs_only_when_invoked() {
    let ran = Arc::new(AtomicBool::new(false));
    let hook_ran = ran.clone();
    let module = DynamicModule::new(ModuleMetadata::new()).with_pre_bootstrap(move || {
        hook_ran.store(true, Ordering::SeqCst);
        Ok(())
    });

    assert!(!ran.load(Ordering::SeqCst));
    ModuleOrchestrator::run_dynamic_pre_bootstrap(&module).unwrap();
    assert!(ran.load(Ordering::SeqCst));
    assert!(module.metadata.providers.is_empty());
}

#[test]
fn dynamic_module_pre_bootstrap_errors_bubble_through_orchestrator_helper() {
    let module = DynamicModule::new(ModuleMetadata::new())
        .with_pre_bootstrap(|| Err("pre-bootstrap refused to run".to_string()));

    let err = ModuleOrchestrator::run_dynamic_pre_bootstrap(&module).unwrap_err();
    assert_eq!(err, "pre-bootstrap refused to run");
}

#[test]
fn dynamic_module_without_pre_bootstrap_callback_is_noop() {
    let module = DynamicModule::new(ModuleMetadata::new());

    assert_eq!(module.run_pre_bootstrap(), Ok(()));
}

#[test]
fn dynamic_module_merged_metadata_preserves_fields_and_deduplicates_providers() {
    let metadata = ModuleMetadata::new()
        .with_imports(vec![TypeId::of::<FeatureDynamicModuleMarker>()])
        .with_providers(vec![TypeId::of::<RootService>()])
        .with_controllers(vec![TypeId::of::<DynamicConsumerModule>()])
        .with_exports(vec![TypeId::of::<RootService>()])
        .with_middlewares(vec![TypeId::of::<DynamicImportingConsumerModule>()])
        .with_global(true);
    let module = DynamicModule::new(metadata.clone()).with_providers(vec![
        TypeId::of::<RootService>(),
        TypeId::of::<FeatureService>(),
    ]);

    let merged = module.merged_metadata();

    assert_eq!(merged.imports, metadata.imports);
    assert_eq!(merged.controllers, metadata.controllers);
    assert_eq!(merged.exports, metadata.exports);
    assert_eq!(merged.middlewares, metadata.middlewares);
    assert!(merged.is_global);
    assert_eq!(
        merged.providers,
        vec![TypeId::of::<RootService>(), TypeId::of::<FeatureService>()]
    );
    assert_eq!(module.metadata.providers, vec![TypeId::of::<RootService>()]);
}

#[test]
fn dynamic_module_clone_keeps_pre_bootstrap_and_latest_provider_list() {
    let runs = Arc::new(AtomicUsize::new(0));
    let cloned_runs = runs.clone();
    let module = DynamicModule::new(ModuleMetadata::new())
        .with_providers(vec![TypeId::of::<RootService>()])
        .with_providers(vec![
            TypeId::of::<FeatureService>(),
            TypeId::of::<FeatureServiceTwo>(),
        ])
        .with_pre_bootstrap(move || {
            cloned_runs.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
    let cloned = module.clone();

    assert_eq!(
        cloned.providers,
        vec![
            TypeId::of::<FeatureService>(),
            TypeId::of::<FeatureServiceTwo>()
        ]
    );

    module.run_pre_bootstrap().unwrap();
    cloned.run_pre_bootstrap().unwrap();

    assert_eq!(runs.load(Ordering::SeqCst), 2);
}

#[test]
fn register_dynamic_module_exposes_root_exports_to_other_consumers_when_global() {
    let mut registry = ModuleRegistry::new();
    registry.register_dynamic::<RootDynamicModuleMarker>(ExampleDynamicModule::for_root(
        DynamicOptions {
            provider: TypeId::of::<RootService>(),
            is_global: true,
        },
    ));
    registry.register(&DynamicConsumerModule);

    let visible = registry.visible_exports::<DynamicConsumerModule>().unwrap();
    assert!(visible.contains(&TypeId::of::<RootService>()));
}

#[test]
fn register_dynamic_module_replacement_updates_exported_surface() {
    let mut registry = ModuleRegistry::new();
    assert!(registry.register_dynamic::<RootDynamicModuleMarker>(
        ExampleDynamicModule::for_root(DynamicOptions {
            provider: TypeId::of::<RootService>(),
            is_global: true,
        }),
    ));
    assert!(!registry.register_dynamic::<RootDynamicModuleMarker>(
        ExampleDynamicModule::for_root(DynamicOptions {
            provider: TypeId::of::<FeatureService>(),
            is_global: true,
        }),
    ));
    registry.register(&DynamicConsumerModule);

    let visible = registry.visible_exports::<DynamicConsumerModule>().unwrap();

    assert!(visible.contains(&TypeId::of::<FeatureService>()));
    assert!(!visible.contains(&TypeId::of::<RootService>()));
}

#[test]
fn register_dynamic_feature_module_requires_explicit_imports() {
    let mut registry = ModuleRegistry::new();
    registry.register_dynamic::<FeatureDynamicModuleMarker>(ExampleDynamicModule::for_feature(
        DynamicOptions {
            provider: TypeId::of::<FeatureService>(),
            is_global: false,
        },
    ));
    registry.register(&DynamicConsumerModule);
    registry.register(&DynamicImportingConsumerModule);

    let consumer_visible = registry.visible_exports::<DynamicConsumerModule>().unwrap();
    assert!(!consumer_visible.contains(&TypeId::of::<FeatureService>()));

    let importing_visible = registry
        .visible_exports::<DynamicImportingConsumerModule>()
        .unwrap();
    assert!(importing_visible.contains(&TypeId::of::<FeatureService>()));
}

#[test]
fn for_feature_dynamic_modules_stay_isolated_per_importing_module() {
    let mut registry = ModuleRegistry::new();
    registry.register_dynamic::<FeatureDynamicModuleMarker>(ExampleDynamicModule::for_feature(
        DynamicOptions {
            provider: TypeId::of::<FeatureService>(),
            is_global: false,
        },
    ));
    registry.register_dynamic::<FeatureDynamicModuleMarkerTwo>(ExampleDynamicModule::for_feature(
        DynamicOptions {
            provider: TypeId::of::<FeatureServiceTwo>(),
            is_global: false,
        },
    ));
    registry.register(&DynamicImportingConsumerModule);
    registry.register(&DynamicImportingConsumerModuleTwo);

    let first_visible = registry
        .visible_exports::<DynamicImportingConsumerModule>()
        .unwrap();
    assert!(first_visible.contains(&TypeId::of::<FeatureService>()));
    assert!(!first_visible.contains(&TypeId::of::<FeatureServiceTwo>()));

    let second_visible = registry
        .visible_exports::<DynamicImportingConsumerModuleTwo>()
        .unwrap();
    assert!(second_visible.contains(&TypeId::of::<FeatureServiceTwo>()));
    assert!(!second_visible.contains(&TypeId::of::<FeatureService>()));
}

#[test]
fn dynamic_module_can_reexport_imported_provider_surface() {
    let mut registry = ModuleRegistry::new();
    registry.register_dynamic::<FeatureDynamicModuleMarker>(ExampleDynamicModule::for_feature(
        DynamicOptions {
            provider: TypeId::of::<ReExportedService>(),
            is_global: false,
        },
    ));
    assert!(registry.register_dynamic::<DynamicReExportModuleMarker>(
        DynamicModule::new(
            ModuleMetadata::new()
                .with_imports(vec![TypeId::of::<FeatureDynamicModuleMarker>()])
                .with_exports(vec![TypeId::of::<ReExportedService>()]),
        ),
    ));
    registry.register(&DynamicReExportConsumerModule);

    let visible = registry
        .visible_exports::<DynamicReExportConsumerModule>()
        .unwrap();

    assert!(visible.contains(&TypeId::of::<ReExportedService>()));
}

#[test]
fn dynamic_module_invalid_reexport_still_fails_registry_validation() {
    let mut registry = ModuleRegistry::new();
    assert!(registry.register_dynamic::<InvalidDynamicModuleMarker>(
        DynamicModule::new(ModuleMetadata::new().with_exports(vec![TypeId::of::<RootService>()])),
    ));

    let err = registry
        .exported_surface_for(TypeId::of::<InvalidDynamicModuleMarker>())
        .unwrap_err();

    assert!(matches!(
        err,
        nivasa_core::module::ModuleRegistryError::InvalidExport {
            exported,
            ..
        } if exported == TypeId::of::<RootService>()
    ));
}
