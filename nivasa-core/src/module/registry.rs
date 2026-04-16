use super::{DynamicModule, Module, ModuleMetadata};
use crate::di::error::DiError;
use std::any::TypeId;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// Registered module metadata plus its concrete Rust identity.
///
/// `ModuleEntry` is what the registry stores after a module is registered.
/// You normally reach it through [`ModuleRegistry::get`] or
/// [`ModuleRegistry::entries`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleEntry {
    /// Concrete Rust type for the registered module.
    pub type_id: TypeId,
    /// Fully qualified Rust type name, used for diagnostics and ordering.
    pub type_name: &'static str,
    /// Captured module metadata.
    pub metadata: ModuleMetadata,
}

/// Errors raised while building or resolving the module dependency graph.
///
/// The registry reports missing imports, invalid exports, and circular import
/// chains before module lookup is used at runtime.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModuleRegistryError {
    #[error("module '{module}' depends on an unregistered module ({missing:?})")]
    MissingImport {
        module: &'static str,
        missing: TypeId,
    },
    #[error("module '{module}' exports {exported:?} but that item is not provided locally or by an import")]
    InvalidExport {
        module: &'static str,
        exported: TypeId,
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
///
/// ```rust,no_run
/// use async_trait::async_trait;
/// use nivasa_core::di::{DependencyContainer, DiError};
/// use nivasa_core::module::{Module, ModuleMetadata, ModuleRegistry};
/// use std::any::TypeId;
///
/// struct LeafService;
/// struct SharedService;
/// struct AppService;
/// struct LeafModule;
/// struct SharedModule;
/// struct AppModule;
///
/// #[async_trait]
/// impl Module for LeafModule {
///     fn metadata(&self) -> ModuleMetadata {
///         ModuleMetadata::new()
///             .with_providers(vec![TypeId::of::<LeafService>()])
///             .with_exports(vec![TypeId::of::<LeafService>()])
///     }
///
///     async fn configure(&self, _container: &DependencyContainer) -> Result<(), DiError> {
///         Ok(())
///     }
/// }
///
/// #[async_trait]
/// impl Module for SharedModule {
///     fn metadata(&self) -> ModuleMetadata {
///         ModuleMetadata::new()
///             .with_imports(vec![TypeId::of::<LeafModule>()])
///             .with_providers(vec![TypeId::of::<SharedService>()])
///             .with_exports(vec![TypeId::of::<SharedService>()])
///     }
///
///     async fn configure(&self, _container: &DependencyContainer) -> Result<(), DiError> {
///         Ok(())
///     }
/// }
///
/// #[async_trait]
/// impl Module for AppModule {
///     fn metadata(&self) -> ModuleMetadata {
///         ModuleMetadata::new()
///             .with_imports(vec![TypeId::of::<SharedModule>()])
///             .with_providers(vec![TypeId::of::<AppService>()])
///     }
///
///     async fn configure(&self, _container: &DependencyContainer) -> Result<(), DiError> {
///         Ok(())
///     }
/// }
///
/// let mut registry = ModuleRegistry::new();
/// registry.register(&LeafModule);
/// registry.register(&SharedModule);
/// registry.register(&AppModule);
///
/// assert!(registry.contains::<AppModule>());
/// assert_eq!(registry.get::<SharedModule>().unwrap().type_name, std::any::type_name::<SharedModule>());
/// assert_eq!(
///     registry.resolve_order().unwrap(),
///     vec![
///         TypeId::of::<LeafModule>(),
///         TypeId::of::<SharedModule>(),
///         TypeId::of::<AppModule>(),
///     ]
/// );
/// assert!(registry.visible_exports::<AppModule>().unwrap().contains(&TypeId::of::<SharedService>()));
/// ```
#[derive(Debug, Default, Clone)]
pub struct ModuleRegistry {
    entries: HashMap<TypeId, ModuleEntry>,
}

impl ModuleRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of registered modules.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the registry has no registered modules.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether `M` has been registered.
    pub fn contains<M: Module>(&self) -> bool {
        self.entries.contains_key(&TypeId::of::<M>())
    }

    /// Fetch the stored entry for `M`.
    pub fn get<M: Module>(&self) -> Option<&ModuleEntry> {
        self.entries.get(&TypeId::of::<M>())
    }

    /// Register a concrete module instance.
    ///
    /// Returns `true` when the module was newly inserted and `false` when an
    /// existing entry for the same type was replaced.
    ///
    /// ```rust,no_run
    /// use async_trait::async_trait;
    /// use nivasa_core::di::{DependencyContainer, DiError};
    /// use nivasa_core::module::{Module, ModuleMetadata, ModuleRegistry};
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
    /// let mut registry = ModuleRegistry::new();
    /// assert!(registry.register(&AppModule));
    /// assert!(!registry.register(&AppModule));
    /// ```
    pub fn register<M: Module>(&mut self, module: &M) -> bool {
        let entry = ModuleEntry {
            type_id: TypeId::of::<M>(),
            type_name: std::any::type_name::<M>(),
            metadata: module.metadata(),
        };
        self.entries.insert(entry.type_id, entry).is_none()
    }

    /// Register a dynamic module under the concrete type `M`.
    pub fn register_dynamic<M: 'static>(&mut self, module: DynamicModule) -> bool {
        let entry = ModuleEntry {
            type_id: TypeId::of::<M>(),
            type_name: std::any::type_name::<M>(),
            metadata: module.merged_metadata(),
        };
        self.entries.insert(entry.type_id, entry).is_none()
    }

    /// Iterate over registered entries.
    pub fn entries(&self) -> impl Iterator<Item = &ModuleEntry> {
        self.entries.values()
    }

    fn exported_surface_by_id(
        &self,
        module: TypeId,
        cache: &mut HashMap<TypeId, HashSet<TypeId>>,
        visiting: &mut HashSet<TypeId>,
        path: &mut Vec<&'static str>,
    ) -> Result<HashSet<TypeId>, ModuleRegistryError> {
        self.public_surface_for(module, cache, visiting, path)
    }

    fn global_module_ids(&self) -> Vec<TypeId> {
        let mut globals: Vec<(&'static str, TypeId)> = self
            .entries
            .values()
            .filter(|entry| entry.metadata.is_global)
            .map(|entry| (entry.type_name, entry.type_id))
            .collect();
        globals.sort_by(|left, right| left.0.cmp(right.0));
        globals.into_iter().map(|(_, type_id)| type_id).collect()
    }

    fn public_surface_for(
        &self,
        current: TypeId,
        cache: &mut HashMap<TypeId, HashSet<TypeId>>,
        visiting: &mut HashSet<TypeId>,
        path: &mut Vec<&'static str>,
    ) -> Result<HashSet<TypeId>, ModuleRegistryError> {
        if let Some(cached) = cache.get(&current) {
            return Ok(cached.clone());
        }

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

        visiting.insert(current);
        path.push(entry.type_name);

        let mut import_surfaces = Vec::new();
        for import in &entry.metadata.imports {
            let imported = self
                .entries
                .get(import)
                .ok_or(ModuleRegistryError::MissingImport {
                    module: entry.type_name,
                    missing: *import,
                })?;

            let surface = self.public_surface_for(*import, cache, visiting, path)?;
            import_surfaces.push((imported.type_name, surface));
        }

        let provided: HashSet<TypeId> = entry.metadata.providers.iter().copied().collect();
        let mut surface = HashSet::new();

        for exported in &entry.metadata.exports {
            if provided.contains(exported)
                || import_surfaces
                    .iter()
                    .any(|(_, imported_surface)| imported_surface.contains(exported))
            {
                surface.insert(*exported);
            } else {
                path.pop();
                visiting.remove(&current);
                return Err(ModuleRegistryError::InvalidExport {
                    module: entry.type_name,
                    exported: *exported,
                });
            }
        }

        path.pop();
        visiting.remove(&current);
        cache.insert(current, surface.clone());

        Ok(surface)
    }

    /// Compute the types this module makes visible to its consumers.
    ///
    /// The visible surface includes the module's own exports, exports from
    /// imported modules, and exports from global modules.
    ///
    /// A module can see:
    /// - Its own exported providers
    /// - Exports from imported modules
    /// - Exports from global modules
    pub fn visible_exports<M: Module>(&self) -> Result<HashSet<TypeId>, ModuleRegistryError> {
        self.visible_exports_by_id(TypeId::of::<M>())
    }

    /// Compute the types made visible by the module identified by `module`.
    pub fn visible_exports_by_id(
        &self,
        module: TypeId,
    ) -> Result<HashSet<TypeId>, ModuleRegistryError> {
        let entry = self
            .entries
            .get(&module)
            .expect("module must be registered before querying visibility");

        let mut cache = HashMap::new();
        let mut visiting = HashSet::new();
        let mut path = Vec::new();
        let mut visible = self.public_surface_for(module, &mut cache, &mut visiting, &mut path)?;

        for import in &entry.metadata.imports {
            let surface = self.public_surface_for(*import, &mut cache, &mut visiting, &mut path)?;
            visible.extend(surface);
        }

        for global in self.global_module_ids() {
            if global == module {
                continue;
            }

            let surface = self.public_surface_for(global, &mut cache, &mut visiting, &mut path)?;
            visible.extend(surface);
        }

        // Keep the module's own exports in scope even if it is global.
        if entry.metadata.is_global {
            let surface = self.public_surface_for(module, &mut cache, &mut visiting, &mut path)?;
            visible.extend(surface);
        }

        Ok(visible)
    }

    /// Returns the import/global module sources a module can consume providers from.
    ///
    /// ```rust,no_run
    /// use async_trait::async_trait;
    /// use nivasa_core::di::{DependencyContainer, DiError};
    /// use nivasa_core::module::{Module, ModuleMetadata, ModuleRegistry};
    /// use std::any::TypeId;
    ///
    /// struct LeafModule;
    /// struct AppModule;
    ///
    /// #[async_trait]
    /// impl Module for LeafModule {
    ///     fn metadata(&self) -> ModuleMetadata {
    ///         ModuleMetadata::new().with_global(true)
    ///     }
    ///
    ///     async fn configure(&self, _container: &DependencyContainer) -> Result<(), DiError> {
    ///         Ok(())
    ///     }
    /// }
    ///
    /// #[async_trait]
    /// impl Module for AppModule {
    ///     fn metadata(&self) -> ModuleMetadata {
    ///         ModuleMetadata::new().with_imports(vec![TypeId::of::<LeafModule>()])
    ///     }
    ///
    ///     async fn configure(&self, _container: &DependencyContainer) -> Result<(), DiError> {
    ///         Ok(())
    ///     }
    /// }
    ///
    /// let mut registry = ModuleRegistry::new();
    /// registry.register(&LeafModule);
    /// registry.register(&AppModule);
    ///
    /// let sources = registry.import_sources_by_id(TypeId::of::<AppModule>()).unwrap();
    /// assert!(sources.contains(&TypeId::of::<LeafModule>()));
    /// ```
    pub fn import_sources_by_id(&self, module: TypeId) -> Result<Vec<TypeId>, ModuleRegistryError> {
        let entry = self
            .entries
            .get(&module)
            .expect("module must be registered before querying import sources");

        let mut sources = Vec::new();
        let mut seen = HashSet::new();

        for import in &entry.metadata.imports {
            if !self.entries.contains_key(import) {
                return Err(ModuleRegistryError::MissingImport {
                    module: entry.type_name,
                    missing: *import,
                });
            }

            if seen.insert(*import) {
                sources.push(*import);
            }
        }

        for global in self.global_module_ids() {
            if global != module && seen.insert(global) {
                sources.push(global);
            }
        }

        Ok(sources)
    }

    /// Returns the exported provider surface for the given module.
    pub fn exported_surface_for(
        &self,
        module: TypeId,
    ) -> Result<HashSet<TypeId>, ModuleRegistryError> {
        let mut cache = HashMap::new();
        let mut visiting = HashSet::new();
        let mut path = Vec::new();
        self.exported_surface_by_id(module, &mut cache, &mut visiting, &mut path)
    }

    /// Returns whether a type exported by `Exported` is visible to `Consumer`.
    ///
    /// ```rust,no_run
    /// use async_trait::async_trait;
    /// use nivasa_core::di::{DependencyContainer, DiError};
    /// use nivasa_core::module::{Module, ModuleMetadata, ModuleRegistry};
    /// use std::any::TypeId;
    ///
    /// struct SharedService;
    /// struct LeafModule;
    /// struct AppModule;
    ///
    /// #[async_trait]
    /// impl Module for LeafModule {
    ///     fn metadata(&self) -> ModuleMetadata {
    ///         ModuleMetadata::new()
    ///             .with_providers(vec![TypeId::of::<SharedService>()])
    ///             .with_exports(vec![TypeId::of::<SharedService>()])
    ///     }
    ///
    ///     async fn configure(&self, _container: &DependencyContainer) -> Result<(), DiError> {
    ///         Ok(())
    ///     }
    /// }
    ///
    /// #[async_trait]
    /// impl Module for AppModule {
    ///     fn metadata(&self) -> ModuleMetadata {
    ///         ModuleMetadata::new().with_imports(vec![TypeId::of::<LeafModule>()])
    ///     }
    ///
    ///     async fn configure(&self, _container: &DependencyContainer) -> Result<(), DiError> {
    ///         Ok(())
    ///     }
    /// }
    ///
    /// let mut registry = ModuleRegistry::new();
    /// registry.register(&LeafModule);
    /// registry.register(&AppModule);
    ///
    /// assert!(registry.is_visible_to::<AppModule, SharedService>().unwrap());
    /// ```
    pub fn is_visible_to<Consumer: Module, Exported: 'static>(
        &self,
    ) -> Result<bool, ModuleRegistryError> {
        Ok(self
            .visible_exports::<Consumer>()?
            .contains(&TypeId::of::<Exported>()))
    }

    /// Resolve modules in dependency order, from leaves to roots.
    ///
    /// ```rust,no_run
    /// use async_trait::async_trait;
    /// use nivasa_core::di::{DependencyContainer, DiError};
    /// use nivasa_core::module::{Module, ModuleMetadata, ModuleRegistry};
    /// use std::any::TypeId;
    ///
    /// struct LeafModule;
    /// struct SharedModule;
    /// struct AppModule;
    ///
    /// #[async_trait]
    /// impl Module for LeafModule {
    ///     fn metadata(&self) -> ModuleMetadata {
    ///         ModuleMetadata::new()
    ///     }
    ///
    ///     async fn configure(&self, _container: &DependencyContainer) -> Result<(), DiError> {
    ///         Ok(())
    ///     }
    /// }
    ///
    /// #[async_trait]
    /// impl Module for SharedModule {
    ///     fn metadata(&self) -> ModuleMetadata {
    ///         ModuleMetadata::new().with_imports(vec![TypeId::of::<LeafModule>()])
    ///     }
    ///
    ///     async fn configure(&self, _container: &DependencyContainer) -> Result<(), DiError> {
    ///         Ok(())
    ///     }
    /// }
    ///
    /// #[async_trait]
    /// impl Module for AppModule {
    ///     fn metadata(&self) -> ModuleMetadata {
    ///         ModuleMetadata::new().with_imports(vec![TypeId::of::<SharedModule>()])
    ///     }
    ///
    ///     async fn configure(&self, _container: &DependencyContainer) -> Result<(), DiError> {
    ///         Ok(())
    ///     }
    /// }
    ///
    /// let mut registry = ModuleRegistry::new();
    /// registry.register(&LeafModule);
    /// registry.register(&SharedModule);
    /// registry.register(&AppModule);
    ///
    /// assert_eq!(
    ///     registry.resolve_order().unwrap(),
    ///     vec![
    ///         TypeId::of::<LeafModule>(),
    ///         TypeId::of::<SharedModule>(),
    ///         TypeId::of::<AppModule>(),
    ///     ]
    /// );
    /// ```
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
