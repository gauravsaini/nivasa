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
/// land. For now it only captures global visibility plus the env file path
/// surface that later loading slices will consume.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigOptions {
    pub is_global: bool,
    pub env_file_paths: Vec<String>,
}

impl ConfigOptions {
    /// Create default config options.
    pub const fn new() -> Self {
        Self {
            is_global: false,
            env_file_paths: Vec::new(),
        }
    }

    /// Mark the config module as globally visible.
    pub const fn with_global(mut self, is_global: bool) -> Self {
        self.is_global = is_global;
        self
    }

    /// Add one env file path to the options surface.
    pub fn with_env_file_path(mut self, path: impl Into<String>) -> Self {
        let path = normalize_env_file_path(path.into());
        if !path.is_empty() {
            self.env_file_paths.push(path);
        }
        self
    }

    /// Replace the env file path surface with multiple paths.
    pub fn with_env_file_paths<I, S>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.env_file_paths = paths
            .into_iter()
            .map(Into::into)
            .map(normalize_env_file_path)
            .filter(|path| !path.is_empty())
            .collect();
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

fn normalize_env_file_path(path: String) -> String {
    path.trim().to_string()
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

    #[test]
    fn config_options_support_one_env_file_path() {
        let options = ConfigOptions::new().with_env_file_path(" .env ");

        assert_eq!(options.env_file_paths, vec![".env".to_string()]);
    }

    #[test]
    fn config_options_support_multiple_env_file_paths() {
        let options = ConfigOptions::new()
            .with_env_file_paths([" .env ", "", " .env.local ", "   ", ".env.production"]);

        assert_eq!(
            options.env_file_paths,
            vec![
                ".env".to_string(),
                ".env.local".to_string(),
                ".env.production".to_string()
            ]
        );
    }

    #[test]
    fn for_root_preserves_env_file_path_options_surface() {
        let options = ConfigOptions::new().with_env_file_paths([".env", ".env.local"]);
        let module = ConfigModule::for_root(options.clone());

        assert_eq!(options.env_file_paths, vec![".env", ".env.local"]);
        assert_eq!(
            module.merged_metadata().providers,
            vec![TypeId::of::<ConfigOptionsProvider>()]
        );
    }

    #[test]
    fn for_feature_preserves_env_file_path_options_surface() {
        let options = ConfigOptions::new().with_env_file_paths([".env", ".env.test"]);
        let module = <ConfigModule as nivasa_core::module::ConfigurableModule>::for_feature(
            options.clone(),
        );

        assert_eq!(options.env_file_paths, vec![".env", ".env.test"]);
        assert_eq!(
            module.merged_metadata().providers,
            vec![TypeId::of::<ConfigOptionsProvider>()]
        );
    }
}
