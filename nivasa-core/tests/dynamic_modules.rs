use async_trait::async_trait;
use nivasa_core::module::{
    ConfigurableModule, DynamicModule, Module, ModuleMetadata, ModuleRegistry,
};
use std::any::TypeId;

struct RootService;
struct FeatureService;
struct ConsumerService;
struct RootDynamicModuleMarker;
struct FeatureDynamicModuleMarker;
struct DynamicConsumerModule;
struct DynamicImportingConsumerModule;

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
