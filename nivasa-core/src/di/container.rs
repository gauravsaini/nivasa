use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::di::error::DiError;
use crate::di::provider::{Provider, ProviderScope};
use crate::di::graph::DependencyGraph;

/// The core Dependency Injection Container.
pub struct DependencyContainer {
    /// The registry of all available providers, keyed by the type they resolve.
    providers: RwLock<HashMap<TypeId, Arc<dyn Provider>>>,
    
    /// Cache of constructed singletons.
    singletons: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl Default for DependencyContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyContainer {
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
            singletons: RwLock::new(HashMap::new()),
        }
    }

    /// Register a provider interface.
    pub async fn register<T: Send + Sync + 'static>(&self, provider: Arc<dyn Provider>) {
        let type_id = TypeId::of::<T>();
        let mut providers = self.providers.write().await;
        providers.insert(type_id, provider);
    }

    /// Register a direct value as a singleton provider.
    pub async fn register_value<T: Send + Sync + 'static>(&self, instance: T) {
        let type_id = TypeId::of::<T>();
        let instance_arc = Arc::new(instance);
        let provider = Arc::new(crate::di::provider::ValueProvider::new_from_arc(instance_arc.clone()));
        let lifecycle_provider = Arc::new(crate::di::provider::LifecycleProvider::new(provider));
        
        let mut providers = self.providers.write().await;
        providers.insert(type_id, lifecycle_provider.clone());

        // Also cache it as a singleton immediately
        let mut singletons = self.singletons.write().await;
        singletons.insert(type_id, instance_arc);
    }

    /// Register a type that implements the `Injectable` trait.
    pub async fn register_injectable<T: crate::di::provider::Injectable>(
        &self, 
        scope: ProviderScope,
        dependencies: Vec<TypeId>
    ) {
        let type_id = TypeId::of::<T>();
        let inner_provider = Arc::new(crate::di::provider::ClassProvider::new(
            scope, 
            dependencies,
            move |container| Box::pin(T::build(container))
        ));
        let provider = Arc::new(crate::di::provider::LifecycleProvider::new(inner_provider));
        
        let mut providers = self.providers.write().await;
        providers.insert(type_id, provider);
    }

    /// Check if a type is registered.
    pub async fn has<T: 'static>(&self) -> bool {
        let type_id = TypeId::of::<T>();
        // Check singletons first (like register_value), then providers
        let singletons = self.singletons.read().await;
        if singletons.contains_key(&type_id) {
            return true;
        }
        let providers = self.providers.read().await;
        providers.contains_key(&type_id)
    }

    /// Resolve an instance of the given type.
    pub async fn resolve<T: Send + Sync + 'static>(&self) -> Result<Arc<T>, DiError> {
        let type_id = TypeId::of::<T>();
        let type_name = std::any::type_name::<T>();

        // 1. Check if we already have it constructed (Singleton)
        {
            let singletons = self.singletons.read().await;
            if let Some(instance) = singletons.get(&type_id) {
                // Downcast
                if let Ok(arc_t) = instance.clone().downcast::<T>() {
                    return Ok(arc_t);
                }
            }
        }

        // 2. We don't have it cached. Find the provider.
        let provider = {
            let providers = self.providers.read().await;
            providers.get(&type_id).cloned()
        };

        if let Some(provider) = provider {
            let scope = provider.metadata().scope;
            
            // Build the instance
            // Note: In a fully SCXML compliant system, this step triggers the `nivasa.provider.scxml` lifecycle
            let instance_any = provider.build(self).await?;

            // Try to downcast
            let arc_t = instance_any
                .downcast::<T>()
                .map_err(|_| DiError::ConstructionFailed(type_name, "Internal error: downcast failed".to_string()))?;

            // 3. Cache it if it's a singleton
            if scope == ProviderScope::Singleton {
                let mut singletons = self.singletons.write().await;
                singletons.insert(type_id, arc_t.clone() as Arc<dyn Any + Send + Sync>);
            }

            // Return it
            Ok(arc_t)
        } else {
            Err(DiError::ProviderNotFound(type_name))
        }
    }

    /// Resolve an optional instance of the given type.
    /// Returns Ok(Some(Arc<T>)) if found, Ok(None) if not registered.
    pub async fn resolve_optional<T: Send + Sync + 'static>(&self) -> Result<Option<Arc<T>>, DiError> {
        if self.has::<T>().await {
            self.resolve::<T>().await.map(Some)
        } else {
            Ok(None)
        }
    }

    /// Freezes registrations, validates the dependency graph for cycles, 
    /// and pre-instantiates all Singletons in topological order.
    pub async fn initialize(&self) -> Result<(), DiError> {
        let mut graph = DependencyGraph::new();

        // 1. Build the graph from all registered providers
        {
            let providers = self.providers.read().await;
            for (type_id, provider) in providers.iter() {
                let meta = provider.metadata();
                graph.add_node(*type_id, meta.type_name, meta.dependencies.clone());
            }
        }

        // 2. Resolve construction order and detect cycles
        let resolution_order = graph.resolve_order()?;

        // 3. Pre-instantiate singletons in topological order
        // Because of the order, we guarantee that when a provider is built,
        // any singleton dependencies it requests from the container are already cached.
        for type_id in resolution_order {
            let provider_opt = {
                let providers = self.providers.read().await;
                providers.get(&type_id).cloned()
            };

            if let Some(provider) = provider_opt {
                if provider.metadata().scope == ProviderScope::Singleton {
                    // Check if already in singletons (e.g. registered via register_value)
                    let is_cached = {
                        let singletons = self.singletons.read().await;
                        singletons.contains_key(&type_id)
                    };
                    
                    if !is_cached {
                        let instance_any = provider.build(self).await?;
                        let mut singletons = self.singletons.write().await;
                        singletons.insert(type_id, instance_any);
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use async_trait::async_trait;
    use crate::di::provider::{ProviderMetadata, FactoryProvider};

    struct DepA;
    struct DepB;
    
    struct TestSingletonFactory {
        counter: Arc<AtomicUsize>,
    }

    #[tokio::test]
    async fn test_singleton_resolution() {
        let container = DependencyContainer::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let c = counter.clone();

        let factory = FactoryProvider::new(
            ProviderScope::Singleton,
            vec![], // No deps
            move |_| {
                let ctr = c.clone();
                Box::pin(async move {
                    ctr.fetch_add(1, Ordering::SeqCst);
                    Ok(DepA)
                })
            }
        );

        container.register::<DepA>(Arc::new(factory)).await;
        
        // Initialize should pre-build the singleton
        container.initialize().await.unwrap();
        
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // Resolving it multiple times should not increment the counter
        let _inst1 = container.resolve::<DepA>().await.unwrap();
        let _inst2 = container.resolve::<DepA>().await.unwrap();
        
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
