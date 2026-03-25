use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use super::provider::{Provider, ProviderMetadata};

#[derive(Clone)]
struct ProviderEntry {
    metadata: ProviderMetadata,
    provider: Arc<dyn Provider>,
}

/// Registry of DI providers keyed by `TypeId`.
///
/// The registry keeps the provider instance and a metadata snapshot together so
/// the container can look up registration details without poking through
/// unrelated caches.
#[derive(Default, Clone)]
pub struct ProviderRegistry {
    entries: HashMap<TypeId, ProviderEntry>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains<T: 'static>(&self) -> bool {
        self.entries.contains_key(&TypeId::of::<T>())
    }

    pub fn insert<T: 'static>(&mut self, provider: Arc<dyn Provider>) -> Option<Arc<dyn Provider>> {
        let type_id = TypeId::of::<T>();
        let entry = ProviderEntry {
            metadata: provider.metadata().clone(),
            provider: provider.clone(),
        };

        self.entries
            .insert(type_id, entry)
            .map(|entry| entry.provider)
    }

    pub fn get<T: 'static>(&self) -> Option<Arc<dyn Provider>> {
        self.entries
            .get(&TypeId::of::<T>())
            .map(|entry| entry.provider.clone())
    }

    pub fn get_by_id(&self, type_id: TypeId) -> Option<Arc<dyn Provider>> {
        self.entries
            .get(&type_id)
            .map(|entry| entry.provider.clone())
    }

    pub fn remove<T: 'static>(&mut self) -> Option<Arc<dyn Provider>> {
        self.entries
            .remove(&TypeId::of::<T>())
            .map(|entry| entry.provider)
    }

    pub fn remove_by_id(&mut self, type_id: TypeId) -> Option<Arc<dyn Provider>> {
        self.entries.remove(&type_id).map(|entry| entry.provider)
    }

    pub fn metadata<T: 'static>(&self) -> Option<ProviderMetadata> {
        self.entries
            .get(&TypeId::of::<T>())
            .map(|entry| entry.metadata.clone())
    }

    pub fn snapshot(&self) -> Vec<(TypeId, Arc<dyn Provider>, ProviderMetadata)> {
        self.entries
            .iter()
            .map(|(type_id, entry)| (*type_id, entry.provider.clone(), entry.metadata.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::di::provider::{ProviderScope, ValueProvider};

    #[derive(Debug)]
    struct Foo;

    #[test]
    fn registry_tracks_provider_metadata_and_lifecycle() {
        let mut registry = ProviderRegistry::new();
        let provider = Arc::new(ValueProvider::new(Foo));

        assert!(registry.insert::<Foo>(provider.clone()).is_none());
        assert!(registry.contains::<Foo>());
        assert_eq!(registry.len(), 1);

        let metadata = registry.metadata::<Foo>().expect("metadata should exist");
        assert_eq!(metadata.type_id, TypeId::of::<Foo>());
        assert_eq!(metadata.scope, ProviderScope::Singleton);

        let fetched = registry.get::<Foo>().expect("provider should exist");
        assert_eq!(fetched.metadata().type_id, TypeId::of::<Foo>());

        let removed = registry
            .remove::<Foo>()
            .expect("provider should be removed");
        assert_eq!(removed.metadata().type_id, TypeId::of::<Foo>());
        assert!(registry.is_empty());
    }
}
