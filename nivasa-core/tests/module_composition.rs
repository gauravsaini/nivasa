use async_trait::async_trait;
use nivasa_core::module::{Module, ModuleMetadata, ModuleRegistry, ModuleRegistryError};
use std::any::{type_name, TypeId};

struct PublicService;
struct HiddenService;
struct GlobalService;

struct SimpleModule;
struct LeafModule;
struct ReExportingModule;
struct ConsumerModule;
struct GlobalModule;
struct BrokenExportModule;

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
