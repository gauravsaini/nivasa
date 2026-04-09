//! # nivasa-config
//!
//! Nivasa framework — config.
//!
//! This crate currently exposes the bootstrap-facing `ConfigModule` marker
//! type. Runtime config loading, `for_root`/`for_feature`, env parsing, and
//! injectable services land in later slices.

use nivasa_core::module::{ConfigurableModule, DynamicModule, ModuleMetadata};
use std::any::TypeId;

/// Bootstrap-only options for the config module dynamic surface.
///
/// This stays intentionally small until env loading and schema validation
/// land. For now it only captures whether the config provider should be made
/// globally visible.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigOptions {
    pub is_global: bool,
}

impl ConfigOptions {
    /// Create default config options.
    pub const fn new() -> Self {
        Self { is_global: false }
    }

    /// Mark the config module as globally visible.
    pub const fn with_global(mut self, is_global: bool) -> Self {
        self.is_global = is_global;
        self
    }
}

/// Marker provider type for bootstrap-time config options metadata.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ConfigOptionsProvider;

/// Minimal public config module marker for the `nivasa-config` crate.
///
/// This type intentionally stays small until the richer configuration runtime
/// lands. It gives downstream crates a concrete public surface to reference
/// without implying that env loading or config service wiring already exists.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ConfigModule;

impl ConfigModule {
    /// Create a new `ConfigModule` marker.
    pub const fn new() -> Self {
        Self
    }

    /// Build the root dynamic config module surface.
    ///
    /// This slice only advertises config-related provider metadata and global
    /// visibility. Actual env loading and `ConfigService` wiring land later.
    pub fn for_root(options: ConfigOptions) -> DynamicModule {
        DynamicModule::new(ModuleMetadata::new())
            .with_providers(vec![TypeId::of::<ConfigOptionsProvider>()])
            .with_global(options.is_global)
    }
}

impl ConfigurableModule for ConfigModule {
    type Options = ConfigOptions;

    fn for_root(options: Self::Options) -> DynamicModule {
        Self::for_root(options)
    }

    fn for_feature(options: Self::Options) -> DynamicModule {
        DynamicModule::new(ModuleMetadata::new())
            .with_providers(vec![TypeId::of::<ConfigOptionsProvider>()])
            .with_global(options.is_global)
    }
}

#[cfg(test)]
mod tests {
    use super::{ConfigModule, ConfigOptions, ConfigOptionsProvider};
    use std::any::TypeId;

    #[test]
    fn config_module_is_constructible() {
        assert_eq!(ConfigModule::new(), ConfigModule);
    }

    #[test]
    fn config_module_supports_the_expected_core_traits() {
        let module = ConfigModule;

        assert_eq!(module, ConfigModule::new());
        assert_eq!(format!("{module:?}"), "ConfigModule");
    }

    #[test]
    fn for_root_registers_config_options_provider_metadata() {
        let module = ConfigModule::for_root(ConfigOptions::new());

        assert_eq!(
            module.providers,
            vec![TypeId::of::<ConfigOptionsProvider>()]
        );
        assert!(!module.metadata.is_global);
        assert_eq!(
            module.merged_metadata().providers,
            vec![TypeId::of::<ConfigOptionsProvider>()]
        );
    }

    #[test]
    fn for_root_can_mark_config_module_global() {
        let module = ConfigModule::for_root(ConfigOptions::new().with_global(true));

        assert!(module.metadata.is_global);
    }

    #[test]
    fn for_feature_registers_config_options_provider_metadata() {
        let module = <ConfigModule as nivasa_core::module::ConfigurableModule>::for_feature(
            ConfigOptions::new(),
        );

        assert_eq!(
            module.providers,
            vec![TypeId::of::<ConfigOptionsProvider>()]
        );
        assert!(!module.metadata.is_global);
        assert_eq!(
            module.merged_metadata().providers,
            vec![TypeId::of::<ConfigOptionsProvider>()]
        );
    }

    #[test]
    fn for_feature_can_mark_config_module_global_when_requested() {
        let module = <ConfigModule as nivasa_core::module::ConfigurableModule>::for_feature(
            ConfigOptions::new().with_global(true),
        );

        assert!(module.metadata.is_global);
    }
}
