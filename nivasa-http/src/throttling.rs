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
