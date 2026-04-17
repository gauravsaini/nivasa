//! # nivasa-config
//!
//! Nivasa framework — config.
//!
//! This crate currently exposes the bootstrap-facing `ConfigModule` marker
//! type plus a manual `ConfigSchema` validation helper. Runtime config
//! loading, `for_root`/`for_feature`, env parsing, and richer config services
//! land in later slices.

use nivasa_core::module::{ConfigurableModule, DynamicModule, ModuleMetadata};
use std::any::TypeId;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::str::FromStr;

/// Bootstrap-only options for the config module dynamic surface.
///
/// This stays intentionally small until env loading and schema validation
/// land. For now it only captures global visibility plus the env file path
/// surface that later loading slices will consume.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigOptions {
    /// Mark the config module as globally visible.
    pub is_global: bool,
    /// Ordered list of `.env` file paths to merge.
    pub env_file_paths: Vec<String>,
    /// Skip `.env` files and read only process environment variables.
    pub ignore_env_file: bool,
    /// Enable `$VAR` and `${VAR}` interpolation in loaded values.
    pub expand_variables: bool,
}

impl ConfigOptions {
    /// Create default config options.
    pub const fn new() -> Self {
        Self {
            is_global: false,
            env_file_paths: Vec::new(),
            ignore_env_file: false,
            expand_variables: false,
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

    /// Enable variable interpolation inside loaded env values.
    pub const fn with_expand_variables(mut self, expand_variables: bool) -> Self {
        self.expand_variables = expand_variables;
        self
    }
}

/// Marker provider type for bootstrap-time config options metadata.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ConfigOptionsProvider;

/// Errors raised by read-only config lookups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigException {
    /// A requested key was not present in the config map.
    MissingKey { key: String },
}

impl std::fmt::Display for ConfigException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingKey { key } => write!(f, "missing config key: {key}"),
        }
    }
}

impl std::error::Error for ConfigException {}

/// One validation issue found in loaded config values.
///
/// This surface stays intentionally narrow for now. It only reports missing
/// required keys for already-loaded config maps and does not imply automatic
/// startup or module-init validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigValidationIssue {
    /// A required key was missing from the loaded config map.
    MissingRequiredKey { key: String },
    /// A value failed schema-level validation.
    InvalidValue {
        key: String,
        value: String,
        expected: String,
    },
}

impl std::fmt::Display for ConfigValidationIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingRequiredKey { key } => write!(f, "missing required config key: {key}"),
            Self::InvalidValue {
                key,
                value,
                expected,
            } => {
                write!(f, "invalid config value for {key}: {value} ({expected})")
            }
        }
    }
}

/// Aggregated validation error for loaded config values.
///
/// This is a manual validation helper result, not a startup/runtime hook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValidationError {
    issues: Vec<ConfigValidationIssue>,
}

impl ConfigValidationError {
    /// Build a validation error from collected issues.
    pub fn new(issues: Vec<ConfigValidationIssue>) -> Self {
        Self { issues }
    }

    /// Borrow collected validation issues.
    pub fn issues(&self) -> &[ConfigValidationIssue] {
        &self.issues
    }

    /// True when no validation issues were collected.
    pub fn is_empty(&self) -> bool {
        self.issues.is_empty()
    }
}

impl std::fmt::Display for ConfigValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut issues = self.issues.iter();
        if let Some(first) = issues.next() {
            write!(f, "{first}")?;
            for issue in issues {
                write!(f, "; {issue}")?;
            }
            Ok(())
        } else {
            write!(f, "config validation failed")
        }
    }
}

impl std::error::Error for ConfigValidationError {}

/// Static schema contract for already-loaded config maps.
///
/// This trait is intentionally explicit and read-only. It lets callers define
/// required keys and optional defaults, then validate an in-memory config map
/// without implying startup-time or module-init validation.
pub trait ConfigSchema {
    /// Required keys for this schema.
    fn required_keys() -> &'static [&'static str] {
        &[]
    }

    /// Optional default key/value pairs for missing entries.
    fn defaults() -> &'static [(&'static str, &'static str)] {
        &[]
    }

    /// Additional explicit validation for already-loaded values.
    ///
    /// Implementations can use this hook to report typed value problems or
    /// other schema-specific issues in the same error aggregate as missing
    /// keys.
    fn validate(_loaded: &BTreeMap<String, String>) -> Vec<ConfigValidationIssue> {
        Vec::new()
    }
}

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
    ///
    /// ```
    /// use nivasa_config::ConfigService;
    ///
    /// let service = ConfigService::new();
    /// assert!(service.values().is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }

    /// Build a config service from an already-loaded key/value map.
    ///
    /// ```
    /// use std::collections::BTreeMap;
    /// use nivasa_config::ConfigService;
    ///
    /// let service = ConfigService::from_values(BTreeMap::from([
    ///     ("PORT".to_string(), "3000".to_string()),
    /// ]));
    /// assert_eq!(service.get_raw("PORT"), Some("3000"));
    /// ```
    pub fn from_values(values: BTreeMap<String, String>) -> Self {
        Self { values }
    }

    /// Borrow a raw config value by key.
    ///
    /// ```
    /// use std::collections::BTreeMap;
    /// use nivasa_config::ConfigService;
    ///
    /// let service = ConfigService::from_values(BTreeMap::from([
    ///     ("HOST".to_string(), "127.0.0.1".to_string()),
    /// ]));
    /// assert_eq!(service.get_raw("HOST"), Some("127.0.0.1"));
    /// ```
    pub fn get_raw(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|value| value.as_str())
    }

    /// Borrow and parse a typed config value by key.
    ///
    /// ```
    /// use std::collections::BTreeMap;
    /// use nivasa_config::ConfigService;
    ///
    /// let service = ConfigService::from_values(BTreeMap::from([
    ///     ("PORT".to_string(), "3000".to_string()),
    /// ]));
    /// assert_eq!(service.get::<i32>("PORT"), Some(3000));
    /// ```
    pub fn get<T>(&self, key: &str) -> Option<T>
    where
        T: FromStr,
    {
        self.get_raw(key)?.parse().ok()
    }

    /// Borrow and parse a typed config value, or fall back to a default.
    ///
    /// ```
    /// use std::collections::BTreeMap;
    /// use nivasa_config::ConfigService;
    ///
    /// let service = ConfigService::from_values(BTreeMap::from([
    ///     ("PORT".to_string(), "3000".to_string()),
    /// ]));
    /// assert_eq!(service.get_or_default("MISSING", 80), 80);
    /// assert_eq!(service.get_or_default("PORT", 80), 3000);
    /// ```
    pub fn get_or_default<T>(&self, key: &str, default: T) -> T
    where
        T: FromStr,
    {
        self.get(key).unwrap_or(default)
    }

    /// Borrow a raw config value, or return a config error if it is missing.
    ///
    /// ```
    /// use std::collections::BTreeMap;
    /// use nivasa_config::ConfigService;
    ///
    /// let service = ConfigService::from_values(BTreeMap::from([
    ///     ("HOST".to_string(), "127.0.0.1".to_string()),
    /// ]));
    /// assert_eq!(service.get_or_throw("HOST").unwrap(), "127.0.0.1");
    /// ```
    pub fn get_or_throw(&self, key: &str) -> Result<String, ConfigException> {
        self.get_raw(key)
            .map(|value| value.to_string())
            .ok_or_else(|| ConfigException::MissingKey {
                key: key.to_string(),
            })
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
    /// The env file could not be read from disk.
    EnvFile(io::Error),
}

impl std::fmt::Display for ConfigLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EnvFile(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ConfigLoadError {}

impl From<io::Error> for ConfigLoadError {
    fn from(value: io::Error) -> Self {
        Self::EnvFile(value)
    }
}

/// Errors raised while building a validated root config module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigBootstrapError {
    /// Loading config sources failed before validation could run.
    Load { message: String },
    /// Schema validation failed for the loaded config map.
    Validation { message: String },
}

impl std::fmt::Display for ConfigBootstrapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Load { message } => write!(f, "config load failed: {message}"),
            Self::Validation { message } => write!(f, "config validation failed: {message}"),
        }
    }
}

impl std::error::Error for ConfigBootstrapError {}

impl ConfigModule {
    fn root_dynamic_module(options: &ConfigOptions) -> DynamicModule {
        DynamicModule::new(ModuleMetadata::new().with_exports(config_export_types()))
            .with_providers(config_provider_types())
            .with_global(options.is_global)
    }

    /// Create a new `ConfigModule` marker.
    pub const fn new() -> Self {
        Self
    }

    /// Build the root dynamic config module surface.
    ///
    /// This slice only advertises config-related provider metadata and global
    /// visibility. Actual env loading and richer `ConfigService` wiring land later.
    pub fn for_root(options: ConfigOptions) -> DynamicModule {
        Self::root_dynamic_module(&options)
    }

    /// Build a root dynamic config module and validate its loaded config
    /// against a static schema before returning it.
    ///
    /// The module also carries a pre-bootstrap callback so the same schema
    /// check can be replayed by the framework later without manual glue.
    pub fn for_root_with_schema<S>(
        options: ConfigOptions,
    ) -> Result<DynamicModule, ConfigBootstrapError>
    where
        S: ConfigSchema,
    {
        let loaded = Self::load_env(&options).map_err(|error| ConfigBootstrapError::Load {
            message: error.to_string(),
        })?;

        let module = Self::root_dynamic_module(&options).with_pre_bootstrap({
            let loaded = loaded.clone();
            move || {
                ConfigModule::validate_schema::<S>(&loaded)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            }
        });

        module
            .run_pre_bootstrap()
            .map_err(|message| ConfigBootstrapError::Validation { message })?;

        Ok(module)
    }

    /// Load configured `.env` files into an in-memory map.
    ///
    /// This intentionally does not mutate process env and merges files in the
    /// configured order. Later files can override earlier keys. If variable
    /// expansion is enabled, values can reference other keys via `$VAR` or
    /// `${VAR}` before process env overlay happens.
    ///
    /// ```
    /// use nivasa_config::{ConfigModule, ConfigOptions};
    ///
    /// let options = ConfigOptions::new().with_ignore_env_file(true);
    /// let loaded = ConfigModule::load_env(&options).unwrap();
    /// assert!(loaded.is_empty());
    /// ```
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

        if options.expand_variables {
            loaded = expand_env_values(&loaded);
        }

        loaded.extend(load_process_env());

        Ok(loaded)
    }

    /// Validate that a loaded config map contains all required keys.
    ///
    /// This helper validates an already-loaded map only. It does not wire
    /// validation into startup, module init, or `for_root`.
    ///
    /// ```
    /// use std::collections::BTreeMap;
    /// use nivasa_config::ConfigModule;
    ///
    /// let loaded = BTreeMap::from([
    ///     ("HOST".to_string(), "127.0.0.1".to_string()),
    ///     ("PORT".to_string(), "3000".to_string()),
    /// ]);
    ///
    /// ConfigModule::validate_required_keys(&loaded, ["HOST", "PORT"]).unwrap();
    /// ```
    pub fn validate_required_keys<I, S>(
        loaded: &BTreeMap<String, String>,
        required_keys: I,
    ) -> Result<(), ConfigValidationError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let issues = collect_missing_required_key_issues(loaded, required_keys);

        if issues.is_empty() {
            return Ok(());
        }

        Err(ConfigValidationError::new(issues))
    }

    /// Validate a loaded config map against a static schema contract.
    ///
    /// Defaults are applied first, then required keys are checked on the
    /// merged in-memory map. This helper is explicit and does not imply any
    /// startup or module-init validation path.
    ///
    /// ```
    /// use std::collections::BTreeMap;
    /// use nivasa_config::{ConfigModule, ConfigSchema};
    ///
    /// struct AppConfig;
    ///
    /// impl ConfigSchema for AppConfig {
    ///     fn required_keys() -> &'static [&'static str] {
    ///         &["HOST", "PORT"]
    ///     }
    ///
    ///     fn defaults() -> &'static [(&'static str, &'static str)] {
    ///         &[("PORT", "3000")]
    ///     }
    /// }
    ///
    /// let loaded = BTreeMap::from([("HOST".to_string(), "127.0.0.1".to_string())]);
    /// let validated = ConfigModule::validate_schema::<AppConfig>(&loaded).unwrap();
    /// assert_eq!(validated.get("HOST").map(String::as_str), Some("127.0.0.1"));
    /// assert_eq!(validated.get("PORT").map(String::as_str), Some("3000"));
    /// ```
    pub fn validate_schema<S>(
        loaded: &BTreeMap<String, String>,
    ) -> Result<BTreeMap<String, String>, ConfigValidationError>
    where
        S: ConfigSchema,
    {
        let mut merged = loaded.clone();
        for (key, value) in S::defaults() {
            merged
                .entry((*key).to_string())
                .or_insert_with(|| (*value).to_string());
        }

        let mut issues = collect_missing_required_key_issues(&merged, S::required_keys());
        issues.extend(S::validate(&merged));

        if !issues.is_empty() {
            return Err(ConfigValidationError::new(issues));
        }

        Ok(merged)
    }
}

impl ConfigurableModule for ConfigModule {
    type Options = ConfigOptions;

    fn for_root(options: Self::Options) -> DynamicModule {
        Self::for_root(options)
    }

    fn for_feature(options: Self::Options) -> DynamicModule {
        Self::root_dynamic_module(&options)
    }
}

fn config_provider_types() -> Vec<TypeId> {
    vec![
        TypeId::of::<ConfigOptionsProvider>(),
        TypeId::of::<ConfigService>(),
    ]
}

fn config_export_types() -> Vec<TypeId> {
    vec![TypeId::of::<ConfigService>()]
}

fn collect_missing_required_key_issues<I, S>(
    loaded: &BTreeMap<String, String>,
    required_keys: I,
) -> Vec<ConfigValidationIssue>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut missing = Vec::new();

    for key in required_keys {
        let key = key.as_ref().trim();
        if key.is_empty() || loaded.contains_key(key) || missing.iter().any(|item| item == key) {
            continue;
        }

        missing.push(key.to_string());
    }

    missing
        .into_iter()
        .map(|key| ConfigValidationIssue::MissingRequiredKey { key })
        .collect()
}

fn normalize_env_file_path(path: String) -> String {
    path.trim().to_string()
}

fn load_env_file(path: impl AsRef<Path>) -> Result<BTreeMap<String, String>, ConfigLoadError> {
    let mut loaded = BTreeMap::new();

    let contents = fs::read_to_string(path)?;

    for line in contents.lines() {
        if let Some((key, value)) = parse_env_line(line) {
            loaded.insert(key, value);
        }
    }

    Ok(loaded)
}

fn load_process_env() -> BTreeMap<String, String> {
    std::env::vars().collect()
}

fn expand_env_values(values: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut expanded = BTreeMap::new();

    for (key, value) in values {
        expanded.insert(key.clone(), expand_env_value(value, values, 0));
    }

    expanded
}

fn expand_env_value(value: &str, values: &BTreeMap<String, String>, depth: usize) -> String {
    if depth > 8 {
        return value.to_string();
    }

    let process_env = load_process_env();
    let bytes = value.as_bytes();
    let mut resolved = String::with_capacity(value.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != b'$' {
            resolved.push(bytes[index] as char);
            index += 1;
            continue;
        }

        if index + 1 >= bytes.len() {
            resolved.push('$');
            break;
        }

        if bytes[index + 1] == b'{' {
            let mut end = index + 2;
            while end < bytes.len() && bytes[end] != b'}' {
                end += 1;
            }

            if end >= bytes.len() {
                resolved.push('$');
                index += 1;
                continue;
            }

            let name = &value[index + 2..end];
            resolved.push_str(
                &lookup_env_value(name, values, &process_env)
                    .map(|value| expand_env_value(&value, values, depth + 1))
                    .unwrap_or_default(),
            );
            index = end + 1;
            continue;
        }

        let first = bytes[index + 1] as char;
        if !is_env_name_start(first) {
            resolved.push('$');
            index += 1;
            continue;
        }

        let mut end = index + 2;
        while end < bytes.len() && is_env_name_continue(bytes[end] as char) {
            end += 1;
        }

        let name = &value[index + 1..end];
        resolved.push_str(
            &lookup_env_value(name, values, &process_env)
                .map(|value| expand_env_value(&value, values, depth + 1))
                .unwrap_or_default(),
        );
        index = end;
    }

    resolved
}

fn lookup_env_value(
    key: &str,
    values: &BTreeMap<String, String>,
    process_env: &BTreeMap<String, String>,
) -> Option<String> {
    process_env
        .get(key)
        .cloned()
        .or_else(|| values.get(key).cloned())
}

fn is_env_name_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_env_name_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn parse_env_line(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    let line = line.strip_prefix("export ").unwrap_or(line);
    let (key, value) = line.split_once('=')?;
    let key = key.trim();
    if key.is_empty() {
        return None;
    }

    let mut value = value.trim().to_string();
    if value.len() >= 2 {
        let first = value.chars().next().unwrap();
        let last = value.chars().last().unwrap();
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            value = value[1..value.len() - 1].to_string();
        }
    }

    Some((key.to_string(), value))
}

#[cfg(test)]
mod tests {
    use super::{
        config_export_types, config_provider_types, ConfigLoadError, ConfigModule, ConfigOptions,
        ConfigSchema, ConfigService, ConfigValidationError, ConfigValidationIssue,
    };
    use std::any::TypeId;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};
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
        assert_eq!(module.metadata.exports, config_export_types());
        assert_eq!(module.merged_metadata().providers, config_provider_types());
        assert_eq!(module.merged_metadata().exports, config_export_types());
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
        assert_eq!(module.metadata.exports, config_export_types());
        assert_eq!(module.merged_metadata().providers, config_provider_types());
        assert_eq!(module.merged_metadata().exports, config_export_types());
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

        assert_eq!(service.get_raw("HOST"), Some("127.0.0.1"));
        assert_eq!(service.get_raw("PORT"), Some("3000"));
        assert_eq!(service.get_raw("MISSING"), None);
        assert_eq!(service.values().len(), 2);
    }

    #[test]
    fn config_service_get_coerces_typed_values() {
        let service = ConfigService::from_values(BTreeMap::from([
            ("PORT".to_string(), "3000".to_string()),
            ("FEATURE_ENABLED".to_string(), "true".to_string()),
            ("BROKEN_PORT".to_string(), "abc".to_string()),
        ]));

        assert_eq!(service.get::<i32>("PORT"), Some(3000));
        assert!(matches!(
            service.get::<bool>("FEATURE_ENABLED"),
            Some(true)
        ));
        assert_eq!(service.get::<i32>("MISSING"), None);
        assert_eq!(service.get::<i32>("BROKEN_PORT"), None);
    }

    #[test]
    fn config_service_get_or_default_falls_back_when_missing_or_invalid() {
        let service = ConfigService::from_values(BTreeMap::from([
            ("PORT".to_string(), "3000".to_string()),
            ("FEATURE_ENABLED".to_string(), "true".to_string()),
            ("BROKEN_PORT".to_string(), "abc".to_string()),
        ]));

        assert_eq!(service.get_or_default("PORT", 80), 3000);
        assert_eq!(service.get_or_default("MISSING", 80), 80);
        assert_eq!(service.get_or_default("BROKEN_PORT", 80), 80);
        assert!(service.get_or_default("FEATURE_ENABLED", false));
    }

    #[test]
    fn config_service_get_or_throw_returns_owned_values_or_errors() {
        let service = ConfigService::from_values(BTreeMap::from([
            ("HOST".to_string(), "127.0.0.1".to_string()),
            ("PORT".to_string(), "3000".to_string()),
        ]));

        assert_eq!(service.get_or_throw("HOST"), Ok("127.0.0.1".to_string()));
        assert_eq!(service.get_or_throw("PORT"), Ok("3000".to_string()));
        assert_eq!(
            service.get_or_throw("MISSING"),
            Err(super::ConfigException::MissingKey {
                key: "MISSING".to_string(),
            })
        );
    }

    #[test]
    fn config_service_supports_dotted_namespace_keys() {
        let service = ConfigService::from_values(BTreeMap::from([
            ("database.host".to_string(), "localhost".to_string()),
            ("database.port".to_string(), "5432".to_string()),
        ]));

        assert_eq!(service.get_raw("database.host"), Some("localhost"));
        assert_eq!(
            service.get::<String>("database.host"),
            Some("localhost".to_string())
        );
        assert_eq!(
            service.get_or_throw("database.port"),
            Ok("5432".to_string())
        );
    }

    #[test]
    fn config_options_support_one_env_file_path() {
        let options = ConfigOptions::new().with_env_file_path(" .env ");

        assert_eq!(options.env_file_paths, vec![".env".to_string()]);
    }

    #[test]
    fn load_env_supports_a_custom_env_file_path() {
        let path = write_temp_env_file("NIVASA_CONFIG_TEST_CUSTOM=enabled\n");
        let options = ConfigOptions::new().with_env_file_path(path.to_string_lossy().to_string());

        let loaded = ConfigModule::load_env(&options).expect("custom env path should load");

        assert_eq!(
            loaded.get("NIVASA_CONFIG_TEST_CUSTOM").map(String::as_str),
            Some("enabled")
        );
    }

    #[test]
    fn config_options_support_multiple_env_file_paths() {
        let options = ConfigOptions::new().with_env_file_paths([
            " .env ",
            "",
            " .env.local ",
            "   ",
            ".env.production",
        ]);

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
        assert_eq!(module.merged_metadata().exports, config_export_types());
    }

    #[test]
    fn for_feature_preserves_env_file_path_options_surface() {
        let options = ConfigOptions::new().with_env_file_paths([".env", ".env.test"]);
        let module =
            <ConfigModule as nivasa_core::module::ConfigurableModule>::for_feature(options.clone());

        assert_eq!(options.env_file_paths, vec![".env", ".env.test"]);
        assert_eq!(module.merged_metadata().providers, config_provider_types());
        assert_eq!(module.merged_metadata().exports, config_export_types());
    }

    #[test]
    fn config_options_support_ignore_env_file_flag() {
        let options = ConfigOptions::new().with_ignore_env_file(true);

        assert!(options.ignore_env_file);
    }

    #[test]
    fn config_options_support_expand_variables_flag() {
        let options = ConfigOptions::new().with_expand_variables(true);

        assert!(options.expand_variables);
    }

    #[test]
    fn for_root_preserves_ignore_env_file_options_surface() {
        let options = ConfigOptions::new().with_ignore_env_file(true);
        let module = ConfigModule::for_root(options.clone());

        assert!(options.ignore_env_file);
        assert_eq!(module.merged_metadata().providers, config_provider_types());
        assert_eq!(module.merged_metadata().exports, config_export_types());
    }

    #[test]
    fn for_feature_preserves_ignore_env_file_options_surface() {
        let options = ConfigOptions::new().with_ignore_env_file(true);
        let module =
            <ConfigModule as nivasa_core::module::ConfigurableModule>::for_feature(options.clone());

        assert!(options.ignore_env_file);
        assert_eq!(module.merged_metadata().providers, config_provider_types());
        assert_eq!(module.merged_metadata().exports, config_export_types());
    }

    struct ConfigConsumerModule;

    impl nivasa_core::module::Module for ConfigConsumerModule {
        fn metadata(&self) -> nivasa_core::module::ModuleMetadata {
            nivasa_core::module::ModuleMetadata::new()
        }

        fn configure<'life0, 'life1, 'async_trait>(
            &'life0 self,
            _container: &'life1 nivasa_core::di::DependencyContainer,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<(), nivasa_core::di::error::DiError>>
                    + Send
                    + 'async_trait,
            >,
        >
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            Self: 'async_trait,
        {
            Box::pin(async { Ok(()) })
        }
    }

    #[test]
    fn global_config_service_is_visible_to_other_modules() {
        let mut registry = nivasa_core::module::ModuleRegistry::new();
        registry.register_dynamic::<ConfigModule>(ConfigModule::for_root(
            ConfigOptions::new().with_global(true),
        ));
        registry.register(&ConfigConsumerModule);

        let visible = registry
            .visible_exports::<ConfigConsumerModule>()
            .expect("global config module should resolve exports");

        assert!(visible.contains(&TypeId::of::<ConfigService>()));
    }

    #[test]
    fn load_env_reads_the_first_configured_env_file() {
        let _env_guard = env_test_lock().lock().unwrap();
        let path = write_temp_env_file(
            "NIVASA_CONFIG_TEST_PORT=3000\nNIVASA_CONFIG_TEST_HOST=127.0.0.1\n",
        );
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
    fn load_env_reads_a_dotenv_file() {
        let _env_guard = env_test_lock().lock().unwrap();
        let path = write_temp_env_file("NIVASA_CONFIG_TEST_NAME=from_dotenv\n");
        let options = ConfigOptions::new().with_env_file_path(path.to_string_lossy().to_string());

        let loaded = ConfigModule::load_env(&options).expect(".env file should load");

        assert_eq!(
            loaded.get("NIVASA_CONFIG_TEST_NAME").map(String::as_str),
            Some("from_dotenv")
        );
    }

    #[test]
    fn load_env_merges_configured_env_files_in_order() {
        let _env_guard = env_test_lock().lock().unwrap();
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
        let _env_guard = env_test_lock().lock().unwrap();
        let key = "NIVASA_CONFIG_TEST_OVERRIDE";
        let path = write_temp_env_file("NIVASA_CONFIG_TEST_OVERRIDE=from_file\n");
        let options = ConfigOptions::new().with_env_file_path(path.to_string_lossy().to_string());

        let loaded = with_env_var(key, "from_process", || {
            ConfigModule::load_env(&options).expect("env loading should succeed")
        });

        assert_eq!(loaded.get(key).map(String::as_str), Some("from_process"));
    }

    #[test]
    fn load_env_expands_variable_references_when_enabled() {
        let _env_guard = env_test_lock().lock().unwrap();
        let path = write_temp_env_file(
            "NIVASA_CONFIG_TEST_HOST=localhost\nNIVASA_CONFIG_TEST_PORT=3000\nNIVASA_CONFIG_TEST_URL=http://$NIVASA_CONFIG_TEST_HOST:$NIVASA_CONFIG_TEST_PORT\n",
        );
        let options = ConfigOptions::new()
            .with_env_file_path(path.to_string_lossy().to_string())
            .with_expand_variables(true);

        let loaded = ConfigModule::load_env(&options).expect("env loading should succeed");

        assert_eq!(
            loaded.get("NIVASA_CONFIG_TEST_URL").map(String::as_str),
            Some("http://localhost:3000")
        );
    }

    #[test]
    fn load_env_respects_ignore_env_file_option() {
        let _env_guard = env_test_lock().lock().unwrap();
        let path = write_temp_env_file("PORT=3000\n");
        let options = ConfigOptions::new()
            .with_env_file_path(path.to_string_lossy().to_string())
            .with_ignore_env_file(true);

        let loaded = ConfigModule::load_env(&options).expect("ignored env files should no-op");

        assert!(loaded.is_empty());
    }

    #[test]
    fn load_env_returns_empty_when_no_env_file_is_configured() {
        let _env_guard = env_test_lock().lock().unwrap();
        let loaded =
            ConfigModule::load_env(&ConfigOptions::new()).expect("missing path should no-op");

        assert!(loaded.is_empty());
    }

    #[test]
    fn config_load_error_wraps_dotenv_failures() {
        let _env_guard = env_test_lock().lock().unwrap();
        let options = ConfigOptions::new().with_env_file_path("/definitely/missing/.env");

        let error = ConfigModule::load_env(&options).expect_err("missing file should error");

        assert!(matches!(error, ConfigLoadError::EnvFile(_)));
    }

    #[test]
    fn validate_required_keys_accepts_loaded_keys() {
        let loaded = BTreeMap::from([
            ("HOST".to_string(), "127.0.0.1".to_string()),
            ("PORT".to_string(), "3000".to_string()),
        ]);

        let result = ConfigModule::validate_required_keys(&loaded, ["HOST", "PORT"]);

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn validate_required_keys_aggregates_missing_keys() {
        let loaded = BTreeMap::from([("HOST".to_string(), "127.0.0.1".to_string())]);

        let error = ConfigModule::validate_required_keys(&loaded, ["HOST", "PORT", "API_KEY"])
            .expect_err("missing keys should fail validation");

        assert_eq!(
            error,
            ConfigValidationError::new(vec![
                ConfigValidationIssue::MissingRequiredKey {
                    key: "PORT".to_string(),
                },
                ConfigValidationIssue::MissingRequiredKey {
                    key: "API_KEY".to_string(),
                },
            ])
        );
        assert_eq!(
            error.to_string(),
            "missing required config key: PORT; missing required config key: API_KEY"
        );
    }

    #[test]
    fn validate_required_keys_ignores_blank_and_duplicate_required_entries() {
        let loaded = BTreeMap::new();

        let error = ConfigModule::validate_required_keys(&loaded, ["", "PORT", " PORT ", "   "])
            .expect_err("missing key should fail validation");

        assert_eq!(
            error.issues(),
            &[ConfigValidationIssue::MissingRequiredKey {
                key: "PORT".to_string(),
            }]
        );
    }

    struct DemoConfigSchema;

    impl ConfigSchema for DemoConfigSchema {
        fn required_keys() -> &'static [&'static str] {
            &["HOST", "PORT"]
        }

        fn defaults() -> &'static [(&'static str, &'static str)] {
            &[("PORT", "3000"), ("SCHEME", "http")]
        }
    }

    #[test]
    fn validate_schema_applies_defaults_without_overriding_loaded_values() {
        let loaded = BTreeMap::from([("HOST".to_string(), "127.0.0.1".to_string())]);

        let validated = ConfigModule::validate_schema::<DemoConfigSchema>(&loaded)
            .expect("schema validation should succeed");

        assert_eq!(validated.get("HOST").map(String::as_str), Some("127.0.0.1"));
        assert_eq!(validated.get("PORT").map(String::as_str), Some("3000"));
        assert_eq!(validated.get("SCHEME").map(String::as_str), Some("http"));
    }

    #[test]
    fn validate_schema_preserves_loaded_values_over_defaults() {
        let loaded = BTreeMap::from([
            ("HOST".to_string(), "localhost".to_string()),
            ("PORT".to_string(), "8080".to_string()),
        ]);

        let validated = ConfigModule::validate_schema::<DemoConfigSchema>(&loaded)
            .expect("schema validation should succeed");

        assert_eq!(validated.get("PORT").map(String::as_str), Some("8080"));
    }

    #[test]
    fn validate_schema_reports_missing_required_keys_after_defaults() {
        let loaded = BTreeMap::new();

        let error = ConfigModule::validate_schema::<DemoConfigSchema>(&loaded)
            .expect_err("missing required key should fail");

        assert_eq!(
            error,
            ConfigValidationError::new(vec![ConfigValidationIssue::MissingRequiredKey {
                key: "HOST".to_string(),
            }])
        );
    }

    fn write_temp_env_file(contents: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("nivasa-config-{unique}-{}.env", std::process::id()));
        fs::write(&path, contents).expect("temp env file should write");
        path
    }

    fn env_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
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
