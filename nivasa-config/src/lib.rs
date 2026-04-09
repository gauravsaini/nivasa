//! # nivasa-config
//!
//! Nivasa framework — config.
//!
//! This crate currently exposes the bootstrap-facing `ConfigModule` marker
//! type. Runtime config loading, `for_root`/`for_feature`, env parsing, and
//! richer config services land in later slices.

use dotenvy::Error as DotenvError;
use nivasa_core::module::{ConfigurableModule, DynamicModule, ModuleMetadata};
use std::collections::BTreeMap;
use std::any::TypeId;
use std::path::Path;

/// Bootstrap-only options for the config module dynamic surface.
///
/// This stays intentionally small until env loading and schema validation
/// land. For now it only captures global visibility plus the env file path
/// surface that later loading slices will consume.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigOptions {
    pub is_global: bool,
    pub env_file_paths: Vec<String>,
    pub ignore_env_file: bool,
}

impl ConfigOptions {
    /// Create default config options.
    pub const fn new() -> Self {
        Self {
            is_global: false,
            env_file_paths: Vec::new(),
            ignore_env_file: false,
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

    /// Ignore `.env` files and rely on process environment only.
    pub const fn with_ignore_env_file(mut self, ignore_env_file: bool) -> Self {
        self.ignore_env_file = ignore_env_file;
        self
    }
}

/// Marker provider type for bootstrap-time config options metadata.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ConfigOptionsProvider;

/// Thin config service surface for in-crate provider metadata.
///
/// This stays intentionally small: it only wraps an in-memory config map and
/// exposes read-only accessors. Loading, coercion, schema validation, and
/// startup wiring remain future work.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigService {
    values: BTreeMap<String, String>,
}

impl ConfigService {
    /// Create an empty config service.
    pub fn new() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }

    /// Build a config service from an already-loaded key/value map.
    pub fn from_values(values: BTreeMap<String, String>) -> Self {
        Self { values }
    }

    /// Borrow a raw config value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|value| value.as_str())
    }

    /// Borrow all config values as a read-only map.
    pub fn values(&self) -> &BTreeMap<String, String> {
        &self.values
    }
}

/// Minimal public config module marker for the `nivasa-config` crate.
///
/// This type intentionally stays small until the richer configuration runtime
/// lands. It gives downstream crates a concrete public surface to reference
/// without implying that env loading or config service wiring already exists.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ConfigModule;

/// Errors raised while loading `.env` config sources.
#[derive(Debug)]
pub enum ConfigLoadError {
    EnvFile(DotenvError),
}

impl std::fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EnvFile(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ConfigLoadError {}

impl From<DotenvError> for ConfigLoadError {
    fn from(value: DotenvError) -> Self {
        Self::EnvFile(value)
    }
}

impl ConfigModule {
    /// Create a new `ConfigModule` marker.
    pub const fn new() -> Self {
        Self
    }

    /// Build the root dynamic config module surface.
    ///
    /// This slice only advertises config-related provider metadata and global
    /// visibility. Actual env loading and richer `ConfigService` wiring land later.
    pub fn for_root(options: ConfigOptions) -> DynamicModule {
        DynamicModule::new(ModuleMetadata::new())
            .with_providers(config_provider_types())
            .with_global(options.is_global)
    }

    /// Load configured `.env` files into an in-memory map.
    ///
    /// This intentionally does not mutate process env and merges files in the
    /// configured order. Later files can override earlier keys.
    pub fn load_env(options: &ConfigOptions) -> Result<BTreeMap<String, String>, ConfigLoadError> {
        if options.ignore_env_file {
            return Ok(BTreeMap::new());
        }

        if options.env_file_paths.is_empty() {
            return Ok(BTreeMap::new());
        }

        let mut loaded = BTreeMap::new();

        for path in &options.env_file_paths {
            loaded.extend(load_env_file(path)?);
        }

        loaded.extend(load_process_env());

        Ok(loaded)
    }
}

impl ConfigurableModule for ConfigModule {
    type Options = ConfigOptions;

    fn for_root(options: Self::Options) -> DynamicModule {
        Self::for_root(options)
    }

    fn for_feature(options: Self::Options) -> DynamicModule {
        DynamicModule::new(ModuleMetadata::new())
            .with_providers(config_provider_types())
            .with_global(options.is_global)
    }
}

fn config_provider_types() -> Vec<TypeId> {
    vec![
        TypeId::of::<ConfigOptionsProvider>(),
        TypeId::of::<ConfigService>(),
    ]
}

fn normalize_env_file_path(path: String) -> String {
    path.trim().to_string()
}

fn load_env_file(path: impl AsRef<Path>) -> Result<BTreeMap<String, String>, ConfigLoadError> {
    let mut loaded = BTreeMap::new();

    for entry in dotenvy::from_path_iter(path)? {
        let (key, value) = entry?;
        loaded.insert(key, value);
    }

    Ok(loaded)
}

fn load_process_env() -> BTreeMap<String, String> {
    std::env::vars().collect()
}

#[cfg(test)]
mod tests {
    use super::{
        config_provider_types, ConfigLoadError, ConfigModule, ConfigOptions, ConfigService,
    };
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

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

        assert_eq!(module.providers, config_provider_types());
        assert!(!module.metadata.is_global);
        assert_eq!(module.merged_metadata().providers, config_provider_types());
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

        assert_eq!(module.providers, config_provider_types());
        assert!(!module.metadata.is_global);
        assert_eq!(module.merged_metadata().providers, config_provider_types());
    }

    #[test]
    fn for_feature_can_mark_config_module_global_when_requested() {
        let module = <ConfigModule as nivasa_core::module::ConfigurableModule>::for_feature(
            ConfigOptions::new().with_global(true),
        );

        assert!(module.metadata.is_global);
    }

    #[test]
    fn config_service_exposes_raw_values_without_coercion() {
        let service = ConfigService::from_values(BTreeMap::from([
            ("HOST".to_string(), "127.0.0.1".to_string()),
            ("PORT".to_string(), "3000".to_string()),
        ]));

        assert_eq!(service.get("HOST"), Some("127.0.0.1"));
        assert_eq!(service.get("PORT"), Some("3000"));
        assert_eq!(service.get("MISSING"), None);
        assert_eq!(service.values().len(), 2);
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
        assert_eq!(module.merged_metadata().providers, config_provider_types());
    }

    #[test]
    fn for_feature_preserves_env_file_path_options_surface() {
        let options = ConfigOptions::new().with_env_file_paths([".env", ".env.test"]);
        let module = <ConfigModule as nivasa_core::module::ConfigurableModule>::for_feature(
            options.clone(),
        );

        assert_eq!(options.env_file_paths, vec![".env", ".env.test"]);
        assert_eq!(module.merged_metadata().providers, config_provider_types());
    }

    #[test]
    fn config_options_support_ignore_env_file_flag() {
        let options = ConfigOptions::new().with_ignore_env_file(true);

        assert!(options.ignore_env_file);
    }

    #[test]
    fn for_root_preserves_ignore_env_file_options_surface() {
        let options = ConfigOptions::new().with_ignore_env_file(true);
        let module = ConfigModule::for_root(options.clone());

        assert!(options.ignore_env_file);
        assert_eq!(module.merged_metadata().providers, config_provider_types());
    }

    #[test]
    fn for_feature_preserves_ignore_env_file_options_surface() {
        let options = ConfigOptions::new().with_ignore_env_file(true);
        let module = <ConfigModule as nivasa_core::module::ConfigurableModule>::for_feature(
            options.clone(),
        );

        assert!(options.ignore_env_file);
        assert_eq!(module.merged_metadata().providers, config_provider_types());
    }

    #[test]
    fn load_env_reads_the_first_configured_env_file() {
        let path = write_temp_env_file("NIVASA_CONFIG_TEST_PORT=3000\nNIVASA_CONFIG_TEST_HOST=127.0.0.1\n");
        let options = ConfigOptions::new().with_env_file_path(path.to_string_lossy().to_string());

        let loaded = ConfigModule::load_env(&options).expect("env file should load");

        assert_eq!(
            loaded.get("NIVASA_CONFIG_TEST_HOST").map(String::as_str),
            Some("127.0.0.1")
        );
        assert_eq!(
            loaded.get("NIVASA_CONFIG_TEST_PORT").map(String::as_str),
            Some("3000")
        );
    }

    #[test]
    fn load_env_merges_configured_env_files_in_order() {
        let base_path = write_temp_env_file(
            "NIVASA_CONFIG_TEST_HOST=127.0.0.1\nNIVASA_CONFIG_TEST_PORT=3000\n",
        );
        let override_path =
            write_temp_env_file("NIVASA_CONFIG_TEST_PORT=8080\nNIVASA_CONFIG_TEST_DEBUG=true\n");
        let options = ConfigOptions::new()
            .with_env_file_path(base_path.to_string_lossy().to_string())
            .with_env_file_path(override_path.to_string_lossy().to_string());

        let loaded = ConfigModule::load_env(&options).expect("env files should load");

        assert_eq!(
            loaded.get("NIVASA_CONFIG_TEST_DEBUG").map(String::as_str),
            Some("true")
        );
        assert_eq!(
            loaded.get("NIVASA_CONFIG_TEST_HOST").map(String::as_str),
            Some("127.0.0.1")
        );
        assert_eq!(
            loaded.get("NIVASA_CONFIG_TEST_PORT").map(String::as_str),
            Some("8080")
        );
    }

    #[test]
    fn load_env_prefers_process_env_over_dotenv_values() {
        let key = "NIVASA_CONFIG_TEST_OVERRIDE";
        let path = write_temp_env_file("NIVASA_CONFIG_TEST_OVERRIDE=from_file\n");
        let options = ConfigOptions::new().with_env_file_path(path.to_string_lossy().to_string());

        let loaded = with_env_var(key, "from_process", || {
            ConfigModule::load_env(&options).expect("env loading should succeed")
        });

        assert_eq!(loaded.get(key).map(String::as_str), Some("from_process"));
    }

    #[test]
    fn load_env_respects_ignore_env_file_option() {
        let path = write_temp_env_file("PORT=3000\n");
        let options = ConfigOptions::new()
            .with_env_file_path(path.to_string_lossy().to_string())
            .with_ignore_env_file(true);

        let loaded = ConfigModule::load_env(&options).expect("ignored env files should no-op");

        assert!(loaded.is_empty());
    }

    #[test]
    fn load_env_returns_empty_when_no_env_file_is_configured() {
        let loaded = ConfigModule::load_env(&ConfigOptions::new()).expect("missing path should no-op");

        assert!(loaded.is_empty());
    }

    #[test]
    fn config_load_error_wraps_dotenv_failures() {
        let options = ConfigOptions::new().with_env_file_path("/definitely/missing/.env");

        let error = ConfigModule::load_env(&options).expect_err("missing file should error");

        assert!(matches!(error, ConfigLoadError::EnvFile(_)));
    }

    fn write_temp_env_file(contents: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "nivasa-config-{unique}-{}.env",
            std::process::id()
        ));
        fs::write(&path, contents).expect("temp env file should write");
        path
    }

    fn with_env_var<F, R>(key: &str, value: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value);

        let result = f();

        match previous {
            Some(previous) => std::env::set_var(key, previous),
            None => std::env::remove_var(key),
        }

        result
    }
}
