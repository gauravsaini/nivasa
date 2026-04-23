use async_trait::async_trait;
use nivasa_core::di::{error::DiError, DependencyContainer};
use nivasa_core::module::{Module, ModuleMetadata, ModuleRegistry};
use std::any::{type_name, TypeId};

struct GlobalService;
struct GlobalSelfModule;

#[async_trait]
impl Module for GlobalSelfModule {
    fn metadata(&self) -> ModuleMetadata {
        ModuleMetadata::new()
            .with_providers(vec![TypeId::of::<GlobalService>()])
            .with_exports(vec![TypeId::of::<GlobalService>()])
            .with_global(true)
    }

    async fn configure(&self, _container: &DependencyContainer) -> Result<(), DiError> {
        Ok(())
    }
}

#[test]
fn global_registry_helpers_keep_self_exports_visible() {
    let mut registry = ModuleRegistry::new();

    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);

    assert!(registry.register(&GlobalSelfModule));
    assert_eq!(registry.len(), 1);
    assert!(!registry.is_empty());
    assert!(registry.contains::<GlobalSelfModule>());

    let entry = registry
        .get::<GlobalSelfModule>()
        .expect("registered module entry should be fetchable");
    assert_eq!(entry.type_name, type_name::<GlobalSelfModule>());
    assert!(entry.metadata.is_global);

    assert!(!registry.register(&GlobalSelfModule));
    assert_eq!(registry.len(), 1);

    let entries: Vec<_> = registry.entries().collect();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].type_name, type_name::<GlobalSelfModule>());

    let visible = registry
        .visible_exports_by_id(TypeId::of::<GlobalSelfModule>())
        .unwrap();

    assert!(visible.contains(&TypeId::of::<GlobalService>()));
    assert!(registry
        .is_visible_to::<GlobalSelfModule, GlobalService>()
        .unwrap());
}
