use nivasa_core::module::{ConfigurableModule, DynamicModule, ModuleMetadata};
use std::any::TypeId;

struct RootService;
struct FeatureService;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DynamicOptions {
    provider: TypeId,
    is_global: bool,
}

struct ExampleDynamicModule;

impl ConfigurableModule for ExampleDynamicModule {
    type Options = DynamicOptions;

    fn for_root(options: Self::Options) -> DynamicModule {
        DynamicModule::new(ModuleMetadata::new().with_global(options.is_global))
            .with_providers(vec![options.provider])
    }

    fn for_feature(options: Self::Options) -> DynamicModule {
        DynamicModule::new(ModuleMetadata::new()).with_providers(vec![options.provider])
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
