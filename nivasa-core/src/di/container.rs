use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::di::error::DiError;
use crate::di::graph::DependencyGraph;
use crate::di::provider::{
    ClassProvider, FactoryProvider, LifecycleProvider, Provider, ProviderMetadata, ProviderScope,
    ValueProvider,
};
use crate::di::registry::ProviderRegistry;
use async_trait::async_trait;

#[derive(Clone)]
struct CachedInstance {
    version: u64,
    value: Arc<dyn Any + Send + Sync>,
}

struct DependencyContainerInner {
    providers: RwLock<ProviderRegistry>,
    singletons: RwLock<HashMap<TypeId, CachedInstance>>,
    versions: RwLock<HashMap<TypeId, u64>>,
}

/// Core dependency injection container.
///
/// Stores provider registrations, singleton cache, and scoped cache.
///
/// # Examples
///
/// Register direct values, then resolve them later:
///
/// ```rust
/// # use nivasa_core::DependencyContainer;
/// # let rt = tokio::runtime::Runtime::new().unwrap();
/// # rt.block_on(async {
/// let container = DependencyContainer::new();
/// container.register_value::<u32>(42).await;
///
/// let value = container.resolve::<u32>().await.unwrap();
/// assert_eq!(*value, 42);
/// # });
/// ```
pub struct DependencyContainer {
    inner: Arc<DependencyContainerInner>,
    scoped: Arc<RwLock<HashMap<TypeId, CachedInstance>>>,
}

struct ImportedValueProvider {
    metadata: ProviderMetadata,
    value: Arc<dyn Any + Send + Sync>,
}

#[async_trait]
impl Provider for ImportedValueProvider {
    fn metadata(&self) -> &ProviderMetadata {
        &self.metadata
    }

    async fn build(
        &self,
        _container: &DependencyContainer,
    ) -> Result<Arc<dyn Any + Send + Sync>, DiError> {
        Ok(self.value.clone())
    }
}

impl Clone for DependencyContainer {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            scoped: Arc::clone(&self.scoped),
        }
    }
}

impl Default for DependencyContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyContainer {
    /// Create empty container.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(DependencyContainerInner {
                providers: RwLock::new(ProviderRegistry::new()),
                singletons: RwLock::new(HashMap::new()),
                versions: RwLock::new(HashMap::new()),
            }),
            scoped: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create child scope.
    ///
    /// Child shares registrations and singleton cache, but keeps own scoped cache.
    pub fn create_scope(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            scoped: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn bump_version(&self, type_id: TypeId) -> u64 {
        let mut versions = self.inner.versions.write().await;
        let next = versions
            .get(&type_id)
            .copied()
            .unwrap_or(0)
            .saturating_add(1);
        versions.insert(type_id, next);
        next
    }

    async fn current_version(&self, type_id: TypeId) -> u64 {
        let versions = self.inner.versions.read().await;
        versions.get(&type_id).copied().unwrap_or(0)
    }

    async fn register_provider<T: Send + Sync + 'static>(&self, provider: Arc<dyn Provider>) {
        let type_id = TypeId::of::<T>();
        self.register_provider_by_id(type_id, provider).await;
    }

    async fn register_provider_by_id(&self, type_id: TypeId, provider: Arc<dyn Provider>) {
        self.bump_version(type_id).await;

        {
            let mut providers = self.inner.providers.write().await;
            providers.insert_by_id(type_id, provider);
        }

        {
            let mut singletons = self.inner.singletons.write().await;
            singletons.remove(&type_id);
        }

        {
            let mut scoped = self.scoped.write().await;
            scoped.remove(&type_id);
        }
    }

    pub(crate) async fn export_singleton_value(
        &self,
        type_id: TypeId,
    ) -> Option<(ProviderMetadata, Arc<dyn Any + Send + Sync>)> {
        let version = self.current_version(type_id).await;
        let metadata = {
            let providers = self.inner.providers.read().await;
            providers.metadata_by_id(type_id)?
        };

        if metadata.scope != ProviderScope::Singleton {
            return None;
        }

        let value = {
            let singletons = self.inner.singletons.read().await;
            let cached = singletons.get(&type_id)?;
            if cached.version != version {
                return None;
            }
            cached.value.clone()
        };

        Some((metadata, value))
    }

    pub(crate) async fn import_singleton_value(
        &self,
        metadata: ProviderMetadata,
        value: Arc<dyn Any + Send + Sync>,
    ) {
        let type_id = metadata.type_id;
        let version = self.bump_version(type_id).await;
        let provider = Arc::new(ImportedValueProvider {
            metadata,
            value: value.clone(),
        });

        {
            let mut providers = self.inner.providers.write().await;
            providers.insert_by_id(type_id, provider);
        }

        {
            let mut singletons = self.inner.singletons.write().await;
            singletons.insert(type_id, CachedInstance { version, value });
        }

        {
            let mut scoped = self.scoped.write().await;
            scoped.remove(&type_id);
        }
    }

    async fn cache_singleton(
        &self,
        type_id: TypeId,
        version: u64,
        instance: Arc<dyn Any + Send + Sync>,
    ) {
        let mut singletons = self.inner.singletons.write().await;
        singletons.insert(
            type_id,
            CachedInstance {
                version,
                value: instance,
            },
        );
    }

    async fn cache_scoped(
        &self,
        type_id: TypeId,
        version: u64,
        instance: Arc<dyn Any + Send + Sync>,
    ) {
        let mut scoped = self.scoped.write().await;
        scoped.insert(
            type_id,
            CachedInstance {
                version,
                value: instance,
            },
        );
    }

    /// Register provider.
    pub async fn register<T: Send + Sync + 'static>(&self, provider: Arc<dyn Provider>) {
        self.register_provider::<T>(provider).await;
    }

    /// Register direct value as singleton.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use nivasa_core::DependencyContainer;
    /// # let rt = tokio::runtime::Runtime::new().unwrap();
    /// # rt.block_on(async {
    /// let container = DependencyContainer::new();
    /// container.register_value::<String>("hello".to_owned()).await;
    /// let value = container.resolve::<String>().await.unwrap();
    /// assert_eq!(value.as_str(), "hello");
    /// # });
    /// ```
    pub async fn register_value<T: Send + Sync + 'static>(&self, instance: T) {
        let type_id = TypeId::of::<T>();
        let version = self.bump_version(type_id).await;
        let instance_arc = Arc::new(instance);
        let provider = Arc::new(ValueProvider::new_from_arc(instance_arc.clone()));

        {
            let mut providers = self.inner.providers.write().await;
            providers.insert::<T>(provider);
        }

        {
            let mut singletons = self.inner.singletons.write().await;
            singletons.insert(
                type_id,
                CachedInstance {
                    version,
                    value: instance_arc,
                },
            );
        }
    }

    /// Register injectable type.
    pub async fn register_injectable<T: crate::di::provider::Injectable>(
        &self,
        scope: ProviderScope,
        dependencies: Vec<TypeId>,
    ) {
        let type_id = TypeId::of::<T>();
        self.bump_version(type_id).await;
        let inner_provider = Arc::new(ClassProvider::new(scope, dependencies, move |container| {
            Box::pin(T::build(container))
        }));

        {
            let mut providers = self.inner.providers.write().await;
            providers.insert::<T>(inner_provider);
        }

        {
            let mut singletons = self.inner.singletons.write().await;
            singletons.remove(&type_id);
        }

        {
            let mut scoped = self.scoped.write().await;
            scoped.remove(&type_id);
        }
    }

    /// Register factory provider.
    ///
    /// Factory gets container so it can resolve dependencies on demand.
    pub async fn register_factory<T, F>(
        &self,
        scope: ProviderScope,
        dependencies: Vec<TypeId>,
        factory: F,
    ) where
        T: Send + Sync + 'static,
        F: for<'a> Fn(
                &'a DependencyContainer,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<T, DiError>> + Send + 'a>,
            > + Send
            + Sync
            + 'static,
    {
        let provider = Arc::new(FactoryProvider::new(scope, dependencies, factory));

        self.register_provider::<T>(provider).await;
    }

    /// Check if type registered.
    pub async fn has<T: 'static>(&self) -> bool {
        let providers = self.inner.providers.read().await;
        providers.contains::<T>()
    }

    /// Remove provider and invalidate cached instances.
    pub async fn remove<T: 'static>(&self) -> bool {
        let type_id = TypeId::of::<T>();
        let removed = {
            let mut providers = self.inner.providers.write().await;
            providers.remove::<T>().is_some()
        };

        if removed {
            self.bump_version(type_id).await;

            let mut singletons = self.inner.singletons.write().await;
            singletons.remove(&type_id);

            let mut scoped = self.scoped.write().await;
            scoped.remove(&type_id);
        }

        removed
    }

    /// Resolve instance by type.
    ///
    /// Returns cached singleton or scoped instance when available.
    pub async fn resolve<T: Send + Sync + 'static>(&self) -> Result<Arc<T>, DiError> {
        let type_id = TypeId::of::<T>();
        let type_name = std::any::type_name::<T>();

        // Always resolve against the current provider registry first so removed
        // providers cannot be resurrected from a stale cache.
        let version = self.current_version(type_id).await;
        let provider = {
            let providers = self.inner.providers.read().await;
            providers.get::<T>()
        };

        let provider = match provider {
            Some(provider) => provider,
            None => return Err(DiError::ProviderNotFound(type_name)),
        };

        match provider.metadata().scope {
            ProviderScope::Singleton => {
                if let Some(cached) = self.read_cached_singleton(type_id, version).await {
                    return self.downcast_cached(cached, type_name);
                }

                let instance_any = self.build_provider_with_lifecycle(provider.clone()).await?;
                let cached_version = self.current_version(type_id).await;
                if cached_version == version {
                    self.cache_singleton(type_id, version, instance_any.clone())
                        .await;
                }

                self.downcast_instance(instance_any, type_name)
            }
            ProviderScope::Scoped => {
                if let Some(cached) = self.read_cached_scoped(type_id, version).await {
                    return self.downcast_cached(cached, type_name);
                }

                let instance_any = self.build_provider_with_lifecycle(provider.clone()).await?;
                let cached_version = self.current_version(type_id).await;
                if cached_version == version {
                    self.cache_scoped(type_id, version, instance_any.clone())
                        .await;
                }

                self.downcast_instance(instance_any, type_name)
            }
            ProviderScope::Transient => {
                let instance_any = self.build_provider_with_lifecycle(provider).await?;
                self.downcast_instance(instance_any, type_name)
            }
        }
    }

    async fn build_provider_with_lifecycle(
        &self,
        provider: Arc<dyn Provider>,
    ) -> Result<Arc<dyn Any + Send + Sync>, DiError> {
        LifecycleProvider::new(provider).build(self).await
    }

    async fn read_cached_singleton(
        &self,
        type_id: TypeId,
        version: u64,
    ) -> Option<Arc<dyn Any + Send + Sync>> {
        let cached = {
            let singletons = self.inner.singletons.read().await;
            singletons.get(&type_id).cloned()
        };

        match cached {
            Some(entry) if entry.version == version => Some(entry.value),
            Some(_) => {
                let mut singletons = self.inner.singletons.write().await;
                singletons.remove(&type_id);
                None
            }
            None => None,
        }
    }

    async fn read_cached_scoped(
        &self,
        type_id: TypeId,
        version: u64,
    ) -> Option<Arc<dyn Any + Send + Sync>> {
        let cached = {
            let scoped = self.scoped.read().await;
            scoped.get(&type_id).cloned()
        };

        match cached {
            Some(entry) if entry.version == version => Some(entry.value),
            Some(_) => {
                let mut scoped = self.scoped.write().await;
                scoped.remove(&type_id);
                None
            }
            None => None,
        }
    }

    fn downcast_cached<T: Send + Sync + 'static>(
        &self,
        cached: Arc<dyn Any + Send + Sync>,
        type_name: &'static str,
    ) -> Result<Arc<T>, DiError> {
        cached.downcast::<T>().map_err(|_| {
            DiError::ConstructionFailed(type_name, "Internal error: downcast failed".to_string())
        })
    }

    fn downcast_instance<T: Send + Sync + 'static>(
        &self,
        instance: Arc<dyn Any + Send + Sync>,
        type_name: &'static str,
    ) -> Result<Arc<T>, DiError> {
        instance.downcast::<T>().map_err(|_| {
            DiError::ConstructionFailed(type_name, "Internal error: downcast failed".to_string())
        })
    }

    /// Resolve optional instance by type.
    ///
    /// Returns `Ok(Some(_))` if registered, `Ok(None)` if not.
    pub async fn resolve_optional<T: Send + Sync + 'static>(
        &self,
    ) -> Result<Option<Arc<T>>, DiError> {
        if self.has::<T>().await {
            self.resolve::<T>().await.map(Some)
        } else {
            Ok(None)
        }
    }

    /// Validate graph and prebuild singleton cache.
    ///
    /// Freezes no API surface, but walks dependency graph to catch cycles early.
    pub async fn initialize(&self) -> Result<(), DiError> {
        let mut graph = DependencyGraph::new();

        // 1. Build the graph from all registered providers
        {
            let providers = self.inner.providers.read().await;
            for (type_id, provider, _) in providers.snapshot() {
                let meta = provider.metadata();
                graph.add_node(type_id, meta.type_name, meta.dependencies.clone());
            }
        }

        // 2. Resolve construction order and detect cycles
        let resolution_order = graph.resolve_order()?;

        // 3. Pre-instantiate singletons in topological order
        // Because of the order, we guarantee that when a provider is built,
        // any singleton dependencies it requests from the container are already cached.
        for type_id in resolution_order {
            let provider_opt = {
                let providers = self.inner.providers.read().await;
                providers.get_by_id(type_id)
            };

            if let Some(provider) = provider_opt {
                if provider.metadata().scope == ProviderScope::Singleton {
                    let version = self.current_version(type_id).await;
                    let is_cached = {
                        let singletons = self.inner.singletons.read().await;
                        singletons
                            .get(&type_id)
                            .map(|entry| entry.version == version)
                            .unwrap_or(false)
                    };

                    if !is_cached {
                        let instance_any =
                            self.build_provider_with_lifecycle(provider.clone()).await?;
                        let latest_version = self.current_version(type_id).await;
                        if latest_version == version {
                            self.cache_singleton(type_id, version, instance_any).await;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::di::provider::FactoryProvider;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct TestValue {
        id: usize,
    }

    #[tokio::test]
    async fn test_singleton_resolution() {
        let container = DependencyContainer::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let factory = FactoryProvider::new(ProviderScope::Singleton, vec![], move |_| {
            let ctr = c.clone();
            Box::pin(async move {
                let id = ctr.fetch_add(1, Ordering::SeqCst);
                Ok(TestValue { id })
            })
        });

        container.register::<TestValue>(Arc::new(factory)).await;
        container.initialize().await.unwrap();

        assert_eq!(counter.load(Ordering::SeqCst), 1);

        let inst1 = container.resolve::<TestValue>().await.unwrap();
        let inst2 = container.resolve::<TestValue>().await.unwrap();

        assert_eq!(inst1.id, inst2.id);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_scoped_resolution_is_per_scope() {
        let container = DependencyContainer::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let factory = FactoryProvider::new(ProviderScope::Scoped, vec![], move |_| {
            let ctr = c.clone();
            Box::pin(async move {
                let id = ctr.fetch_add(1, Ordering::SeqCst);
                Ok(TestValue { id })
            })
        });

        container.register::<TestValue>(Arc::new(factory)).await;

        let scope_a = container.create_scope();
        let scope_b = container.create_scope();

        let a1 = scope_a.resolve::<TestValue>().await.unwrap();
        let a2 = scope_a.resolve::<TestValue>().await.unwrap();
        let b1 = scope_b.resolve::<TestValue>().await.unwrap();

        assert_eq!(a1.id, a2.id);
        assert_ne!(a1.id, b1.id);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_transient_resolution_creates_new_instances() {
        let container = DependencyContainer::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let factory = FactoryProvider::new(ProviderScope::Transient, vec![], move |_| {
            let ctr = c.clone();
            Box::pin(async move {
                let id = ctr.fetch_add(1, Ordering::SeqCst);
                Ok(TestValue { id })
            })
        });

        container.register::<TestValue>(Arc::new(factory)).await;

        let first = container.resolve::<TestValue>().await.unwrap();
        let second = container.resolve::<TestValue>().await.unwrap();

        assert_ne!(first.id, second.id);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_remove_invalidates_existing_scope_cache() {
        let container = DependencyContainer::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let first_factory = FactoryProvider::new(ProviderScope::Scoped, vec![], move |_| {
            let ctr = c.clone();
            Box::pin(async move {
                let id = ctr.fetch_add(1, Ordering::SeqCst);
                Ok(TestValue { id })
            })
        });

        container
            .register::<TestValue>(Arc::new(first_factory))
            .await;

        let scope = container.create_scope();
        let first = scope.resolve::<TestValue>().await.unwrap();
        assert_eq!(first.id, 0);

        assert!(container.remove::<TestValue>().await);
        assert!(!container.has::<TestValue>().await);

        let second_factory = FactoryProvider::new(ProviderScope::Scoped, vec![], move |_| {
            Box::pin(async move { Ok(TestValue { id: 99 }) })
        });

        container
            .register::<TestValue>(Arc::new(second_factory))
            .await;

        let second = scope.resolve::<TestValue>().await.unwrap();
        assert_eq!(second.id, 99);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
