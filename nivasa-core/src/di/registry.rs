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
///
/// # Example
///
/// ```rust
/// use std::sync::Arc;
///
/// use nivasa_core::di::provider::{ProviderScope, ValueProvider};
/// use nivasa_core::di::registry::ProviderRegistry;
///
/// #[derive(Debug)]
/// struct Config;
///
/// let mut registry = ProviderRegistry::new();
/// let provider = Arc::new(ValueProvider::new(Config));
///
/// assert!(registry.insert::<Config>(provider.clone()).is_none());
/// assert!(registry.contains::<Config>());
/// assert_eq!(registry.len(), 1);
///
/// let metadata = registry.metadata::<Config>().expect("metadata");
/// assert_eq!(metadata.scope, ProviderScope::Singleton);
///
/// let fetched = registry.get::<Config>().expect("provider");
/// assert_eq!(fetched.metadata().scope, ProviderScope::Singleton);
///
/// let snapshot = registry.snapshot();
/// assert_eq!(snapshot.len(), 1);
///
/// let removed = registry.remove::<Config>().expect("removed");
/// assert_eq!(removed.metadata().scope, ProviderScope::Singleton);
/// assert!(registry.is_empty());
/// ```
#[derive(Default, Clone)]
pub struct ProviderRegistry {
    entries: HashMap<TypeId, ProviderEntry>,
}

impl ProviderRegistry {
    /// Creates an empty provider registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of registered providers.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when no providers are registered.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns `true` when a provider for `T` is registered.
    pub fn contains<T: 'static>(&self) -> bool {
        self.entries.contains_key(&TypeId::of::<T>())
    }

    /// Inserts a provider for `T` and returns the previous provider, if any.
    pub fn insert<T: 'static>(&mut self, provider: Arc<dyn Provider>) -> Option<Arc<dyn Provider>> {
        let type_id = TypeId::of::<T>();
        self.insert_by_id(type_id, provider)
    }

    /// Inserts a provider for an explicit `TypeId`.
    ///
    /// ```rust
    /// use std::any::TypeId;
    /// use std::sync::Arc;
    ///
    /// use nivasa_core::di::provider::{ProviderScope, ValueProvider};
    /// use nivasa_core::di::registry::ProviderRegistry;
    ///
    /// #[derive(Debug)]
    /// struct Config;
    ///
    /// let mut registry = ProviderRegistry::new();
    /// let provider = Arc::new(ValueProvider::new(Config));
    /// let type_id = TypeId::of::<Config>();
    ///
    /// assert!(registry.insert_by_id(type_id, provider.clone()).is_none());
    /// assert!(registry.get_by_id(type_id).is_some());
    ///
    /// let metadata = registry.metadata_by_id(type_id).expect("metadata");
    /// assert_eq!(metadata.scope, ProviderScope::Singleton);
    /// ```
    pub fn insert_by_id(
        &mut self,
        type_id: TypeId,
        provider: Arc<dyn Provider>,
    ) -> Option<Arc<dyn Provider>> {
        let entry = ProviderEntry {
            metadata: provider.metadata().clone(),
            provider: provider.clone(),
        };

        self.entries
            .insert(type_id, entry)
            .map(|entry| entry.provider)
    }

    /// Returns the provider registered for `T`, if any.
    pub fn get<T: 'static>(&self) -> Option<Arc<dyn Provider>> {
        self.entries
            .get(&TypeId::of::<T>())
            .map(|entry| entry.provider.clone())
    }

    /// Returns the provider registered for `type_id`, if any.
    ///
    /// ```rust
    /// use std::any::TypeId;
    /// use std::sync::Arc;
    ///
    /// use nivasa_core::di::provider::ValueProvider;
    /// use nivasa_core::di::registry::ProviderRegistry;
    ///
    /// #[derive(Debug)]
    /// struct Config;
    ///
    /// let mut registry = ProviderRegistry::new();
    /// let provider = Arc::new(ValueProvider::new(Config));
    /// let type_id = TypeId::of::<Config>();
    ///
    /// registry.insert_by_id(type_id, provider.clone());
    /// assert!(registry.get_by_id(type_id).is_some());
    /// ```
    pub fn get_by_id(&self, type_id: TypeId) -> Option<Arc<dyn Provider>> {
        self.entries
            .get(&type_id)
            .map(|entry| entry.provider.clone())
    }

    /// Removes and returns the provider registered for `T`, if any.
    pub fn remove<T: 'static>(&mut self) -> Option<Arc<dyn Provider>> {
        self.entries
            .remove(&TypeId::of::<T>())
            .map(|entry| entry.provider)
    }

    /// Removes and returns the provider registered for `type_id`, if any.
    ///
    /// ```rust
    /// use std::any::TypeId;
    /// use std::sync::Arc;
    ///
    /// use nivasa_core::di::provider::ValueProvider;
    /// use nivasa_core::di::registry::ProviderRegistry;
    ///
    /// #[derive(Debug)]
    /// struct Config;
    ///
    /// let mut registry = ProviderRegistry::new();
    /// let provider = Arc::new(ValueProvider::new(Config));
    /// let type_id = TypeId::of::<Config>();
    ///
    /// registry.insert_by_id(type_id, provider);
    /// assert!(registry.remove_by_id(type_id).is_some());
    /// assert!(registry.get_by_id(type_id).is_none());
    /// assert!(registry.metadata_by_id(type_id).is_none());
    /// ```
    pub fn remove_by_id(&mut self, type_id: TypeId) -> Option<Arc<dyn Provider>> {
        self.entries.remove(&type_id).map(|entry| entry.provider)
    }

    /// Returns the metadata snapshot for `T`, if any.
    pub fn metadata<T: 'static>(&self) -> Option<ProviderMetadata> {
        self.entries
            .get(&TypeId::of::<T>())
            .map(|entry| entry.metadata.clone())
    }

    /// Returns the metadata snapshot for `type_id`, if any.
    ///
    /// ```rust
    /// use std::any::TypeId;
    /// use std::sync::Arc;
    ///
    /// use nivasa_core::di::provider::{ProviderScope, ValueProvider};
    /// use nivasa_core::di::registry::ProviderRegistry;
    ///
    /// #[derive(Debug)]
    /// struct Config;
    ///
    /// let mut registry = ProviderRegistry::new();
    /// let provider = Arc::new(ValueProvider::new(Config));
    /// let type_id = TypeId::of::<Config>();
    ///
    /// registry.insert_by_id(type_id, provider);
    /// let metadata = registry.metadata_by_id(type_id).expect("metadata");
    /// assert_eq!(metadata.scope, ProviderScope::Singleton);
    /// ```
    pub fn metadata_by_id(&self, type_id: TypeId) -> Option<ProviderMetadata> {
        self.entries.get(&type_id).map(|entry| entry.metadata.clone())
    }

    /// Returns a snapshot of all registered providers and their metadata.
    ///
    /// ```rust
    /// use std::any::TypeId;
    /// use std::sync::Arc;
    ///
    /// use nivasa_core::di::provider::ValueProvider;
    /// use nivasa_core::di::registry::ProviderRegistry;
    ///
    /// #[derive(Debug)]
    /// struct Config;
    ///
    /// let mut registry = ProviderRegistry::new();
    /// let provider = Arc::new(ValueProvider::new(Config));
    /// registry.insert_by_id(TypeId::of::<Config>(), provider);
    ///
    /// let snapshot = registry.snapshot();
    /// assert_eq!(snapshot.len(), 1);
    /// assert_eq!(snapshot[0].0, TypeId::of::<Config>());
    /// ```
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
