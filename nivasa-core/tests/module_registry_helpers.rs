use async_trait::async_trait;
use nivasa_core::di::{DependencyContainer, error::DiError};
use nivasa_core::module::{
    DynamicModule, Module, ModuleMetadata, ModuleRegistry, ModuleRegistryError,
};
use std::any::{type_name, TypeId};

struct SharedService;
struct RootProvider;

struct AlphaGlobalModule;
struct BetaGlobalModule;
struct SharedImportModule;
struct DynamicConsumerModule;
struct MissingImportConsumerModule;
struct DynamicProviderModuleMarker;

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
impl Module for AlphaGlobalModule {
    fn metadata(&self) -> ModuleMetadata {
        metadata(
            vec![],
            vec![TypeId::of::<RootProvider>()],
            vec![TypeId::of::<RootProvider>()],
            true,
        )
    }

    async fn configure(&self, _container: &DependencyContainer) -> Result<(), DiError> {
        Ok(())
    }
}

#[async_trait]
impl Module for BetaGlobalModule {
    fn metadata(&self) -> ModuleMetadata {
        metadata(
            vec![],
            vec![TypeId::of::<SharedService>()],
            vec![TypeId::of::<SharedService>()],
            true,
        )
    }

    async fn configure(&self, _container: &DependencyContainer) -> Result<(), DiError> {
        Ok(())
    }
}

#[async_trait]
impl Module for SharedImportModule {
    fn metadata(&self) -> ModuleMetadata {
        metadata(
            vec![],
            vec![TypeId::of::<SharedService>()],
            vec![TypeId::of::<SharedService>()],
            false,
        )
    }

    async fn configure(&self, _container: &DependencyContainer) -> Result<(), DiError> {
        Ok(())
    }
}

#[async_trait]
impl Module for DynamicConsumerModule {
    fn metadata(&self) -> ModuleMetadata {
        metadata(
            vec![
                TypeId::of::<SharedImportModule>(),
                TypeId::of::<AlphaGlobalModule>(),
            ],
            vec![],
            vec![],
            false,
        )
    }

    async fn configure(&self, _container: &DependencyContainer) -> Result<(), DiError> {
        Ok(())
    }
}

#[async_trait]
impl Module for MissingImportConsumerModule {
    fn metadata(&self) -> ModuleMetadata {
        metadata(
            vec![TypeId::of::<SharedImportModule>()],
            vec![],
            vec![],
            false,
        )
    }

    async fn configure(&self, _container: &DependencyContainer) -> Result<(), DiError> {
        Ok(())
    }
}

#[test]
fn import_sources_deduplicate_explicit_globals_and_append_other_globals() {
    let mut registry = ModuleRegistry::new();
    registry.register(&AlphaGlobalModule);
    registry.register(&BetaGlobalModule);
    registry.register(&SharedImportModule);
    registry.register(&DynamicConsumerModule);

    let sources = registry
        .import_sources_by_id(TypeId::of::<DynamicConsumerModule>())
        .unwrap();

    assert_eq!(
        sources,
        vec![
            TypeId::of::<SharedImportModule>(),
            TypeId::of::<AlphaGlobalModule>(),
            TypeId::of::<BetaGlobalModule>(),
        ]
    );
}

#[test]
fn import_sources_report_missing_imports_before_resolution_walk() {
    let mut registry = ModuleRegistry::new();
    registry.register(&MissingImportConsumerModule);

    let err = registry
        .import_sources_by_id(TypeId::of::<MissingImportConsumerModule>())
        .unwrap_err();

    match err {
        ModuleRegistryError::MissingImport { module, missing } => {
            assert_eq!(module, type_name::<MissingImportConsumerModule>());
            assert_eq!(missing, TypeId::of::<SharedImportModule>());
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn exported_surface_uses_dynamic_module_merged_provider_metadata() {
    let mut registry = ModuleRegistry::new();
    let dynamic_module = DynamicModule::new(
        ModuleMetadata::new().with_exports(vec![TypeId::of::<RootProvider>()]),
    )
    .with_providers(vec![TypeId::of::<RootProvider>()]);

    assert!(registry.register_dynamic::<DynamicProviderModuleMarker>(dynamic_module));

    let exported = registry
        .exported_surface_for(TypeId::of::<DynamicProviderModuleMarker>())
        .unwrap();

    assert!(exported.contains(&TypeId::of::<RootProvider>()));
}
