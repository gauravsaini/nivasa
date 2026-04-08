use nivasa::prelude::*;
use std::any::TypeId;
use std::future::Future;
use std::pin::Pin;

struct PublicService;
struct HiddenService;

struct LeafModule;
struct ReExportingModule;
struct ConsumerModule;

fn metadata(imports: Vec<TypeId>, providers: Vec<TypeId>, exports: Vec<TypeId>) -> ModuleMetadata {
    ModuleMetadata::new()
        .with_imports(imports)
        .with_providers(providers)
        .with_exports(exports)
}

impl Module for LeafModule {
    fn metadata(&self) -> ModuleMetadata {
        metadata(
            vec![],
            vec![TypeId::of::<PublicService>(), TypeId::of::<HiddenService>()],
            vec![TypeId::of::<PublicService>()],
        )
    }

    fn configure<'life0, 'life1, 'async_trait>(
        &'life0 self,
        _container: &'life1 DependencyContainer,
    ) -> Pin<Box<dyn Future<Output = Result<(), DiError>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async { Ok(()) })
    }
}

impl Module for ReExportingModule {
    fn metadata(&self) -> ModuleMetadata {
        metadata(
            vec![TypeId::of::<LeafModule>()],
            vec![],
            vec![TypeId::of::<PublicService>()],
        )
    }

    fn configure<'life0, 'life1, 'async_trait>(
        &'life0 self,
        _container: &'life1 DependencyContainer,
    ) -> Pin<Box<dyn Future<Output = Result<(), DiError>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async { Ok(()) })
    }
}

impl Module for ConsumerModule {
    fn metadata(&self) -> ModuleMetadata {
        metadata(vec![TypeId::of::<ReExportingModule>()], vec![], vec![])
    }

    fn configure<'life0, 'life1, 'async_trait>(
        &'life0 self,
        _container: &'life1 DependencyContainer,
    ) -> Pin<Box<dyn Future<Output = Result<(), DiError>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async { Ok(()) })
    }
}

#[test]
fn umbrella_crate_module_composition_respects_nested_imports_and_exports() {
    let mut registry = ModuleRegistry::new();
    registry.register(&LeafModule);
    registry.register(&ReExportingModule);
    registry.register(&ConsumerModule);

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
