use super::{Module, ModuleMetadata};
use crate::di::error::DiError;
use std::any::TypeId;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// Registered module metadata plus its concrete Rust identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleEntry {
    pub type_id: TypeId,
    pub type_name: &'static str,
    pub metadata: ModuleMetadata,
}

/// Errors raised while building or resolving the module dependency graph.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModuleRegistryError {
    #[error("module '{module}' depends on an unregistered module ({missing:?})")]
    MissingImport {
        module: &'static str,
        missing: TypeId,
    },
    #[error("module '{module}' is part of a circular import chain: {cycle}")]
    CircularImport { module: &'static str, cycle: String },
}

impl From<ModuleRegistryError> for DiError {
    fn from(value: ModuleRegistryError) -> Self {
        DiError::Registration(value.to_string())
    }
}

/// Registry of known modules and their dependency graph.
#[derive(Debug, Default, Clone)]
pub struct ModuleRegistry {
    entries: HashMap<TypeId, ModuleEntry>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains<M: Module>(&self) -> bool {
        self.entries.contains_key(&TypeId::of::<M>())
    }

    pub fn get<M: Module>(&self) -> Option<&ModuleEntry> {
        self.entries.get(&TypeId::of::<M>())
    }

    pub fn register<M: Module>(&mut self, module: &M) -> bool {
        let entry = ModuleEntry {
            type_id: TypeId::of::<M>(),
            type_name: std::any::type_name::<M>(),
            metadata: module.metadata(),
        };
        self.entries.insert(entry.type_id, entry).is_none()
    }

    pub fn entries(&self) -> impl Iterator<Item = &ModuleEntry> {
        self.entries.values()
    }

    pub fn resolve_order(&self) -> Result<Vec<TypeId>, ModuleRegistryError> {
        let mut order = Vec::new();
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();

        let mut roots: Vec<(&'static str, TypeId)> = self
            .entries
            .values()
            .map(|entry| (entry.type_name, entry.type_id))
            .collect();
        roots.sort_by(|left, right| left.0.cmp(right.0));

        for (_, type_id) in roots {
            if !visited.contains(&type_id) {
                let mut path = Vec::new();
                self.dfs(type_id, &mut visited, &mut visiting, &mut order, &mut path)?;
            }
        }

        Ok(order)
    }

    fn dfs(
        &self,
        current: TypeId,
        visited: &mut HashSet<TypeId>,
        visiting: &mut HashSet<TypeId>,
        order: &mut Vec<TypeId>,
        path: &mut Vec<&'static str>,
    ) -> Result<(), ModuleRegistryError> {
        let entry = self
            .entries
            .get(&current)
            .expect("current module must exist");

        if visiting.contains(&current) {
            path.push(entry.type_name);
            let cycle = path.join(" -> ");
            path.pop();
            return Err(ModuleRegistryError::CircularImport {
                module: entry.type_name,
                cycle,
            });
        }

        if visited.contains(&current) {
            return Ok(());
        }

        visiting.insert(current);
        path.push(entry.type_name);

        for import in &entry.metadata.imports {
            let imported = self
                .entries
                .get(import)
                .ok_or(ModuleRegistryError::MissingImport {
                    module: entry.type_name,
                    missing: *import,
                })?;

            if visiting.contains(import) {
                path.push(imported.type_name);
                let cycle = path.join(" -> ");
                path.pop();
                visiting.remove(&current);
                path.pop();
                return Err(ModuleRegistryError::CircularImport {
                    module: imported.type_name,
                    cycle,
                });
            }

            self.dfs(*import, visited, visiting, order, path)?;
        }

        path.pop();
        visiting.remove(&current);
        visited.insert(current);
        order.push(current);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct RootModule;
    struct SharedModule;
    struct LeafModule;
    struct CyclicA;
    struct CyclicB;
    struct CyclicC;

    fn metadata(imports: Vec<TypeId>) -> ModuleMetadata {
        ModuleMetadata::new().with_imports(imports)
    }

    #[async_trait]
    impl Module for RootModule {
        fn metadata(&self) -> ModuleMetadata {
            metadata(vec![TypeId::of::<SharedModule>()])
        }

        async fn configure(
            &self,
            _container: &crate::di::DependencyContainer,
        ) -> Result<(), crate::di::error::DiError> {
            Ok(())
        }
    }

    #[async_trait]
    impl Module for SharedModule {
        fn metadata(&self) -> ModuleMetadata {
            metadata(vec![TypeId::of::<LeafModule>()])
        }

        async fn configure(
            &self,
            _container: &crate::di::DependencyContainer,
        ) -> Result<(), crate::di::error::DiError> {
            Ok(())
        }
    }

    #[async_trait]
    impl Module for LeafModule {
        fn metadata(&self) -> ModuleMetadata {
            metadata(vec![])
        }

        async fn configure(
            &self,
            _container: &crate::di::DependencyContainer,
        ) -> Result<(), crate::di::error::DiError> {
            Ok(())
        }
    }

    #[async_trait]
    impl Module for CyclicA {
        fn metadata(&self) -> ModuleMetadata {
            metadata(vec![TypeId::of::<CyclicB>()])
        }

        async fn configure(
            &self,
            _container: &crate::di::DependencyContainer,
        ) -> Result<(), crate::di::error::DiError> {
            Ok(())
        }
    }

    #[async_trait]
    impl Module for CyclicB {
        fn metadata(&self) -> ModuleMetadata {
            metadata(vec![TypeId::of::<CyclicC>()])
        }

        async fn configure(
            &self,
            _container: &crate::di::DependencyContainer,
        ) -> Result<(), crate::di::error::DiError> {
            Ok(())
        }
    }

    #[async_trait]
    impl Module for CyclicC {
        fn metadata(&self) -> ModuleMetadata {
            metadata(vec![TypeId::of::<CyclicA>()])
        }

        async fn configure(
            &self,
            _container: &crate::di::DependencyContainer,
        ) -> Result<(), crate::di::error::DiError> {
            Ok(())
        }
    }

    #[test]
    fn test_registry_orders_modules_from_leaf_to_root() {
        let mut registry = ModuleRegistry::new();
        registry.register(&RootModule);
        registry.register(&SharedModule);
        registry.register(&LeafModule);

        let order = registry.resolve_order().unwrap();

        assert_eq!(
            order,
            vec![
                TypeId::of::<LeafModule>(),
                TypeId::of::<SharedModule>(),
                TypeId::of::<RootModule>(),
            ]
        );
    }

    #[test]
    fn test_registry_reports_missing_import() {
        struct MissingImportModule;

        #[async_trait]
        impl Module for MissingImportModule {
            fn metadata(&self) -> ModuleMetadata {
                ModuleMetadata::new().with_imports(vec![TypeId::of::<LeafModule>()])
            }

            async fn configure(
                &self,
                _container: &crate::di::DependencyContainer,
            ) -> Result<(), crate::di::error::DiError> {
                Ok(())
            }
        }

        let mut registry = ModuleRegistry::new();
        registry.register(&MissingImportModule);

        let err = registry.resolve_order().unwrap_err();
        match err {
            ModuleRegistryError::MissingImport { module, .. } => {
                assert_eq!(module, std::any::type_name::<MissingImportModule>());
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn test_registry_detects_circular_imports() {
        let mut registry = ModuleRegistry::new();
        registry.register(&CyclicA);
        registry.register(&CyclicB);
        registry.register(&CyclicC);

        let err = registry.resolve_order().unwrap_err();
        match err {
            ModuleRegistryError::CircularImport { cycle, .. } => {
                assert!(
                    cycle.contains("CyclicA")
                        && cycle.contains("CyclicB")
                        && cycle.contains("CyclicC")
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
