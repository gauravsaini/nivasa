use super::ModuleMetadata;
use std::any::TypeId;

/// Dynamic module metadata plus additional provider registrations.
///
/// ```rust
/// use nivasa_core::module::{DynamicModule, ModuleMetadata};
/// use std::any::TypeId;
///
/// struct CacheService;
///
/// let module = DynamicModule::new(ModuleMetadata::new())
///     .with_providers(vec![TypeId::of::<CacheService>()])
///     .with_global(true);
///
/// assert!(module.metadata.is_global);
/// assert_eq!(module.providers, vec![TypeId::of::<CacheService>()]);
/// ```
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DynamicModule {
    pub metadata: ModuleMetadata,
    pub providers: Vec<TypeId>,
}

impl DynamicModule {
    /// Creates a dynamic module from base metadata.
    ///
    /// ```rust
    /// use nivasa_core::module::{DynamicModule, ModuleMetadata};
    ///
    /// let module = DynamicModule::new(ModuleMetadata::new());
    ///
    /// assert!(module.providers.is_empty());
    /// ```
    pub fn new(metadata: ModuleMetadata) -> Self {
        Self {
            metadata,
            providers: Vec::new(),
        }
    }

    /// Replaces the provider list attached to the module.
    ///
    /// ```rust
    /// use nivasa_core::module::{DynamicModule, ModuleMetadata};
    /// use std::any::TypeId;
    ///
    /// struct PaymentGateway;
    /// struct BillingService;
    ///
    /// let module = DynamicModule::new(ModuleMetadata::new())
    ///     .with_providers(vec![TypeId::of::<PaymentGateway>(), TypeId::of::<BillingService>()]);
    ///
    /// assert_eq!(module.providers.len(), 2);
    /// ```
    pub fn with_providers(mut self, providers: Vec<TypeId>) -> Self {
        self.providers = providers;
        self
    }

    /// Marks the module as global or local.
    ///
    /// ```rust
    /// use nivasa_core::module::{DynamicModule, ModuleMetadata};
    ///
    /// let module = DynamicModule::new(ModuleMetadata::new()).with_global(true);
    ///
    /// assert!(module.metadata.is_global);
    /// ```
    pub fn with_global(mut self, is_global: bool) -> Self {
        self.metadata = self.metadata.with_global(is_global);
        self
    }

    /// Merges the attached provider list into the metadata snapshot.
    ///
    /// ```rust
    /// use nivasa_core::module::{DynamicModule, ModuleMetadata};
    /// use std::any::TypeId;
    ///
    /// struct AuditService;
    ///
    /// let module = DynamicModule::new(ModuleMetadata::new())
    ///     .with_providers(vec![TypeId::of::<AuditService>()]);
    ///
    /// let merged = module.merged_metadata();
    /// assert!(merged.providers.contains(&TypeId::of::<AuditService>()));
    /// ```
    pub fn merged_metadata(&self) -> ModuleMetadata {
        let mut metadata = self.metadata.clone();
        for provider in &self.providers {
            if !metadata.providers.contains(provider) {
                metadata.providers.push(*provider);
            }
        }
        metadata
    }
}

/// Factory trait for modules that expose runtime-configurable metadata.
///
/// ```rust
/// use nivasa_core::module::{ConfigurableModule, DynamicModule, ModuleMetadata};
///
/// struct CacheOptions {
///     global: bool,
/// }
///
/// struct CacheModule;
///
/// impl ConfigurableModule for CacheModule {
///     type Options = CacheOptions;
///
///     fn for_root(options: Self::Options) -> DynamicModule {
///         DynamicModule::new(ModuleMetadata::new()).with_global(options.global)
///     }
///
///     fn for_feature(_options: Self::Options) -> DynamicModule {
///         DynamicModule::new(ModuleMetadata::new())
///     }
/// }
///
/// let module = CacheModule::for_root(CacheOptions { global: true });
/// assert!(module.metadata.is_global);
/// ```
pub trait ConfigurableModule {
    type Options;

    /// Builds a module for application-wide registration.
    fn for_root(options: Self::Options) -> DynamicModule;

    /// Builds a module for feature-scoped registration.
    fn for_feature(options: Self::Options) -> DynamicModule;
}
