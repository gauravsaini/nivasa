use nivasa_core::module::{ConfigurableModule, DynamicModule, ModuleMetadata};
pub use nivasa_guards::{InMemoryThrottlerStorage, ThrottlerGuard, ThrottlerStorage};
use std::any::TypeId;
use std::time::Duration;

/// Bootstrap-facing options for rate limiting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThrottlerOptions {
    pub is_global: bool,
    pub limit: u32,
    pub ttl: Duration,
}

impl ThrottlerOptions {
    /// Create throttler options with the given limit window.
    pub fn new(limit: u32, ttl: Duration) -> Self {
        Self {
            is_global: false,
            limit,
            ttl,
        }
    }

    /// Mark throttling as global.
    pub fn with_global(mut self, is_global: bool) -> Self {
        self.is_global = is_global;
        self
    }
}

impl Default for ThrottlerOptions {
    fn default() -> Self {
        Self::new(10, Duration::from_secs(60))
    }
}

/// Marker type for bootstrap-time throttling options metadata.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ThrottlerOptionsProvider;

/// Dynamic module marker for request throttling.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ThrottlerModule;

impl ThrottlerModule {
    /// Create a new throttling module marker.
    pub const fn new() -> Self {
        Self
    }

    /// Build a root throttling module surface.
    pub fn for_root(options: ThrottlerOptions) -> DynamicModule {
        DynamicModule::new(ModuleMetadata::new().with_exports(throttler_export_types()))
            .with_providers(throttler_provider_types())
            .with_global(options.is_global)
    }
}

impl ConfigurableModule for ThrottlerModule {
    type Options = ThrottlerOptions;

    fn for_root(options: Self::Options) -> DynamicModule {
        ThrottlerModule::for_root(options)
    }

    fn for_feature(options: Self::Options) -> DynamicModule {
        DynamicModule::new(ModuleMetadata::new().with_exports(throttler_export_types()))
            .with_providers(throttler_provider_types())
            .with_global(options.is_global)
    }
}

fn throttler_provider_types() -> Vec<TypeId> {
    vec![
        TypeId::of::<ThrottlerOptionsProvider>(),
        TypeId::of::<InMemoryThrottlerStorage>(),
        TypeId::of::<ThrottlerGuard>(),
    ]
}

fn throttler_export_types() -> Vec<TypeId> {
    vec![
        TypeId::of::<InMemoryThrottlerStorage>(),
        TypeId::of::<ThrottlerGuard>(),
    ]
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn throttler_options_new_sets_defaults() {
        let opts = ThrottlerOptions::new(5, Duration::from_secs(30));
        assert_eq!(opts.limit, 5);
        assert_eq!(opts.ttl, Duration::from_secs(30));
        assert!(!opts.is_global);
    }

    #[test]
    fn throttler_options_with_global_toggles_flag() {
        let opts = ThrottlerOptions::new(10, Duration::from_secs(60)).with_global(true);
        assert!(opts.is_global);

        let opts2 = opts.with_global(false);
        assert!(!opts2.is_global);
    }

    #[test]
    fn throttler_options_default_is_10_per_60s() {
        let opts = ThrottlerOptions::default();
        assert_eq!(opts.limit, 10);
        assert_eq!(opts.ttl, Duration::from_secs(60));
        assert!(!opts.is_global);
    }

    #[test]
    fn throttler_options_provider_default_is_debug_and_clone() {
        let provider = ThrottlerOptionsProvider::default();
        let _ = format!("{provider:?}");
        let _cloned = provider;
    }

    #[test]
    fn throttler_module_new_creates_marker() {
        let _m = ThrottlerModule::new();
        let _default = ThrottlerModule::default();
        let _ = format!("{:?}", ThrottlerModule);
    }

    #[test]
    fn throttler_module_for_root_produces_global_module() {
        let opts = ThrottlerOptions::new(5, Duration::from_secs(10)).with_global(true);
        let module = ThrottlerModule::for_root(opts);
        assert!(module.metadata.is_global);
    }

    #[test]
    fn throttler_module_for_root_non_global() {
        let opts = ThrottlerOptions::new(5, Duration::from_secs(10));
        let module = ThrottlerModule::for_root(opts);
        assert!(!module.metadata.is_global);
    }

    #[test]
    fn throttler_module_for_feature_via_configurable_module() {
        use super::ConfigurableModule;
        let opts = ThrottlerOptions::new(3, Duration::from_secs(5)).with_global(false);
        let module = ThrottlerModule::for_feature(opts);
        assert!(!module.metadata.is_global);
    }

    #[test]
    fn throttler_module_for_root_via_configurable_module() {
        use super::ConfigurableModule;
        let opts = ThrottlerOptions::new(3, Duration::from_secs(5)).with_global(true);
        let module = ThrottlerModule::for_root(opts);
        assert!(module.metadata.is_global);
    }

    #[test]
    fn throttler_provider_types_contains_three_entries() {
        assert_eq!(throttler_provider_types().len(), 3);
    }

    #[test]
    fn throttler_export_types_contains_two_entries() {
        assert_eq!(throttler_export_types().len(), 2);
    }
}
