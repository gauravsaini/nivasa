use super::ModuleMetadata;
use std::any::TypeId;
use std::fmt;
use std::sync::Arc;

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
pub type DynamicModulePreBootstrap = Arc<dyn Fn() -> Result<(), String> + Send + Sync>;

#[derive(Default)]
pub struct DynamicModule {
    pub metadata: ModuleMetadata,
    pub providers: Vec<TypeId>,
    pre_bootstrap: Option<DynamicModulePreBootstrap>,
}

impl fmt::Debug for DynamicModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynamicModule")
            .field("metadata", &self.metadata)
            .field("providers", &self.providers)
            .field("has_pre_bootstrap", &self.pre_bootstrap.is_some())
            .finish()
    }
}

impl Clone for DynamicModule {
    fn clone(&self) -> Self {
        Self {
            metadata: self.metadata.clone(),
            providers: self.providers.clone(),
            pre_bootstrap: self.pre_bootstrap.clone(),
        }
    }
}

impl PartialEq for DynamicModule {
    fn eq(&self, other: &Self) -> bool {
        self.metadata == other.metadata
            && self.providers == other.providers
            && self.pre_bootstrap.is_some() == other.pre_bootstrap.is_some()
    }
}

impl Eq for DynamicModule {}

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
            pre_bootstrap: None,
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

    /// Attach a callback that can run before module bootstrap.
    ///
    /// The callback is intentionally explicit and side-effect free from the
    /// lifecycle engine's point of view. Callers decide when to run it, so SCXML
    /// module init / activate semantics stay untouched.
    pub fn with_pre_bootstrap<F>(mut self, pre_bootstrap: F) -> Self
    where
        F: Fn() -> Result<(), String> + Send + Sync + 'static,
    {
        self.pre_bootstrap = Some(Arc::new(pre_bootstrap));
        self
    }

    /// Run the optional pre-bootstrap callback.
    pub fn run_pre_bootstrap(&self) -> Result<(), String> {
        if let Some(pre_bootstrap) = &self.pre_bootstrap {
            pre_bootstrap()
        } else {
            Ok(())
        }
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
