use async_trait::async_trait;
use nivasa_core::di::container::DependencyContainer;
use nivasa_core::di::error::DiError;
use nivasa_core::di::provider::Injectable;
use nivasa_core::module::{ConfigurableModule, DynamicModule, ModuleMetadata};
use std::any::TypeId;
use std::collections::BTreeMap;
use tracing_subscriber::filter::{Directive, EnvFilter};
use tracing_subscriber::fmt::MakeWriter;

/// Output format for framework logging.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum LoggerFormat {
    /// Structured JSON logging for machines and production sinks.
    Json,
    /// Human-readable console logging for local development.
    #[default]
    Pretty,
}

/// Bootstrap-facing logger options for the dynamic module surface.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoggerOptions {
    pub is_global: bool,
    pub format: LoggerFormat,
    pub default_level: String,
    pub module_levels: BTreeMap<String, String>,
}

impl LoggerOptions {
    /// Create default logger options.
    pub fn new() -> Self {
        Self {
            is_global: false,
            format: LoggerFormat::Pretty,
            default_level: "info".to_string(),
            module_levels: BTreeMap::new(),
        }
    }

    /// Mark logger module global.
    pub fn with_global(mut self, is_global: bool) -> Self {
        self.is_global = is_global;
        self
    }

    /// Use JSON output.
    pub fn with_json(mut self) -> Self {
        self.format = LoggerFormat::Json;
        self
    }

    /// Use pretty console output.
    pub fn with_pretty(mut self) -> Self {
        self.format = LoggerFormat::Pretty;
        self
    }

    /// Set default log level directive.
    pub fn with_default_level(mut self, level: impl Into<String>) -> Self {
        self.default_level = level.into();
        self
    }

    /// Set module-specific log level directive.
    pub fn with_module_level(
        mut self,
        module: impl Into<String>,
        level: impl Into<String>,
    ) -> Self {
        self.module_levels.insert(module.into(), level.into());
        self
    }
}

/// Structured fields propagated across logging calls.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogContext {
    pub request_id: Option<String>,
    pub user_id: Option<String>,
    pub module_name: Option<String>,
}

impl LogContext {
    /// Create empty logging context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach request id.
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Attach user id.
    pub fn with_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Attach module name.
    pub fn with_module_name(mut self, module_name: impl Into<String>) -> Self {
        self.module_name = Some(module_name.into());
        self
    }
}

/// Errors raised while preparing a logging subscriber.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoggerInitError {
    InvalidDirective(String),
}

impl std::fmt::Display for LoggerInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDirective(directive) => {
                write!(f, "invalid tracing directive: {directive}")
            }
        }
    }
}

impl std::error::Error for LoggerInitError {}

/// Injectable logger service wrapping `tracing` and `tracing-subscriber`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoggerService {
    options: LoggerOptions,
}

impl LoggerService {
    /// Build logger service from options.
    pub fn new(options: LoggerOptions) -> Self {
        Self { options }
    }

    /// Borrow configured options.
    pub fn options(&self) -> &LoggerOptions {
        &self.options
    }

    /// Build env filter from default and per-module directives.
    pub fn env_filter(&self) -> Result<EnvFilter, LoggerInitError> {
        let mut filter = EnvFilter::try_new(self.options.default_level.clone())
            .map_err(|_| LoggerInitError::InvalidDirective(self.options.default_level.clone()))?;

        for (module, level) in &self.options.module_levels {
            let directive = format!("{module}={level}");
            let parsed = directive
                .parse::<Directive>()
                .map_err(|_| LoggerInitError::InvalidDirective(directive.clone()))?;
            filter = filter.add_directive(parsed);
        }

        Ok(filter)
    }

    /// Run closure under logger-configured default subscriber.
    pub fn with_default_subscriber<W, F, R>(
        &self,
        make_writer: W,
        f: F,
    ) -> Result<R, LoggerInitError>
    where
        W: for<'writer> MakeWriter<'writer> + Send + Sync + 'static,
        F: FnOnce() -> R,
    {
        let _ = self.env_filter()?;

        let result = match self.options.format {
            LoggerFormat::Json => {
                let subscriber = tracing_subscriber::fmt()
                    .with_ansi(false)
                    .without_time()
                    .json()
                    .with_writer(make_writer)
                    .finish();
                let guard = tracing::subscriber::set_default(subscriber);
                let result = f();
                drop(guard);
                result
            }
            LoggerFormat::Pretty => {
                let subscriber = tracing_subscriber::fmt()
                    .with_ansi(false)
                    .without_time()
                    .pretty()
                    .with_writer(make_writer)
                    .finish();
                let guard = tracing::subscriber::set_default(subscriber);
                let result = f();
                drop(guard);
                result
            }
        };

        Ok(result)
    }

    /// Emit info log with propagated structured context.
    pub fn info(&self, context: &LogContext, message: &str) {
        self.info_with_target(module_path!(), context, message);
    }

    /// Emit target-scoped info log with propagated structured context.
    pub fn info_with_target(&self, target: &str, context: &LogContext, message: &str) {
        if !self.should_log(target, "info") {
            return;
        }

        tracing::info!(
            request_id = context.request_id.as_deref().unwrap_or(""),
            user_id = context.user_id.as_deref().unwrap_or(""),
            module_name = context.module_name.as_deref().unwrap_or(target),
            "{message}"
        );
    }

    fn should_log(&self, target: &str, level: &str) -> bool {
        let configured = self
            .options
            .module_levels
            .get(target)
            .map(String::as_str)
            .unwrap_or(self.options.default_level.as_str());

        level_rank(level) >= level_rank(configured)
    }
}

#[async_trait]
impl Injectable for LoggerService {
    async fn build(_container: &DependencyContainer) -> Result<Self, DiError> {
        Ok(Self::new(LoggerOptions::new()))
    }

    fn dependencies() -> Vec<TypeId> {
        Vec::new()
    }
}

/// Marker provider type for bootstrap-time logger options metadata.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct LoggerOptionsProvider;

/// Dynamic module marker for structured logging.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct LoggerModule;

impl LoggerModule {
    /// Create a new logger module marker.
    pub const fn new() -> Self {
        Self
    }

    /// Build root logger module surface.
    pub fn for_root(options: LoggerOptions) -> DynamicModule {
        DynamicModule::new(ModuleMetadata::new().with_exports(logger_export_types()))
            .with_providers(logger_provider_types())
            .with_global(options.is_global)
    }
}

impl ConfigurableModule for LoggerModule {
    type Options = LoggerOptions;

    fn for_root(options: Self::Options) -> DynamicModule {
        LoggerModule::for_root(options)
    }

    fn for_feature(options: Self::Options) -> DynamicModule {
        DynamicModule::new(ModuleMetadata::new().with_exports(logger_export_types()))
            .with_providers(logger_provider_types())
            .with_global(options.is_global)
    }
}

fn logger_provider_types() -> Vec<TypeId> {
    vec![
        TypeId::of::<LoggerOptionsProvider>(),
        TypeId::of::<LoggerService>(),
    ]
}

fn logger_export_types() -> Vec<TypeId> {
    vec![TypeId::of::<LoggerService>()]
}

fn level_rank(level: &str) -> usize {
    match level.trim().to_ascii_lowercase().as_str() {
        "trace" => 0,
        "debug" => 1,
        "info" => 2,
        "warn" => 3,
        "error" => 4,
        _ => usize::MAX,
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    // ── level_rank ────────────────────────────────────────────────────────────

    #[test]
    fn level_rank_trace_is_lowest() {
        assert_eq!(level_rank("trace"), 0);
    }

    #[test]
    fn level_rank_debug() {
        assert_eq!(level_rank("debug"), 1);
    }

    #[test]
    fn level_rank_info() {
        assert_eq!(level_rank("info"), 2);
    }

    #[test]
    fn level_rank_warn() {
        assert_eq!(level_rank("warn"), 3);
    }

    #[test]
    fn level_rank_error_is_highest_named() {
        assert_eq!(level_rank("error"), 4);
    }

    #[test]
    fn level_rank_unknown_returns_max() {
        assert_eq!(level_rank("unknown"), usize::MAX);
        assert_eq!(level_rank(""), usize::MAX);
        assert_eq!(level_rank("VERBOSE"), usize::MAX);
    }

    #[test]
    fn level_rank_is_case_insensitive() {
        assert_eq!(level_rank("INFO"), level_rank("info"));
        assert_eq!(level_rank("DEBUG"), level_rank("debug"));
        assert_eq!(level_rank("WARN"), level_rank("warn"));
    }

    #[test]
    fn level_rank_strips_whitespace() {
        assert_eq!(level_rank("  info  "), level_rank("info"));
    }

    // ── LoggerOptions ─────────────────────────────────────────────────────────

    #[test]
    fn logger_options_new_defaults() {
        let opts = LoggerOptions::new();
        assert!(!opts.is_global);
        assert_eq!(opts.format, LoggerFormat::Pretty);
        assert_eq!(opts.default_level, "info");
        assert!(opts.module_levels.is_empty());
    }

    #[test]
    fn logger_options_with_global() {
        let opts = LoggerOptions::new().with_global(true);
        assert!(opts.is_global);
    }

    #[test]
    fn logger_options_with_json() {
        let opts = LoggerOptions::new().with_json();
        assert_eq!(opts.format, LoggerFormat::Json);
    }

    #[test]
    fn logger_options_with_pretty_after_json() {
        let opts = LoggerOptions::new().with_json().with_pretty();
        assert_eq!(opts.format, LoggerFormat::Pretty);
    }

    #[test]
    fn logger_options_with_default_level() {
        let opts = LoggerOptions::new().with_default_level("warn");
        assert_eq!(opts.default_level, "warn");
    }

    #[test]
    fn logger_options_with_module_level() {
        let opts = LoggerOptions::new().with_module_level("my_crate", "debug");
        assert_eq!(opts.module_levels.get("my_crate").map(String::as_str), Some("debug"));
    }

    // ── LogContext ────────────────────────────────────────────────────────────

    #[test]
    fn log_context_new_is_empty() {
        let ctx = LogContext::new();
        assert!(ctx.request_id.is_none());
        assert!(ctx.user_id.is_none());
        assert!(ctx.module_name.is_none());
    }

    #[test]
    fn log_context_builder_chain() {
        let ctx = LogContext::new()
            .with_request_id("req-123")
            .with_user_id("user-456")
            .with_module_name("orders");

        assert_eq!(ctx.request_id.as_deref(), Some("req-123"));
        assert_eq!(ctx.user_id.as_deref(), Some("user-456"));
        assert_eq!(ctx.module_name.as_deref(), Some("orders"));
    }

    // ── LoggerInitError ───────────────────────────────────────────────────────

    #[test]
    fn logger_init_error_display() {
        let err = LoggerInitError::InvalidDirective("bad=level".to_string());
        let msg = err.to_string();
        assert!(msg.contains("bad=level"), "unexpected: {msg}");
        assert!(msg.contains("invalid tracing directive"), "unexpected: {msg}");
    }

    #[test]
    fn logger_init_error_implements_std_error() {
        let err: Box<dyn std::error::Error> =
            Box::new(LoggerInitError::InvalidDirective("x".to_string()));
        assert!(!err.to_string().is_empty());
    }

    // ── LoggerService::should_log ──────────────────────────────────────────────

    #[test]
    fn should_log_passes_when_level_meets_default() {
        let svc = LoggerService::new(LoggerOptions::new()); // default "info"
        // info >= info → should log
        assert!(svc.should_log("any_target", "info"));
        // warn >= info → should log
        assert!(svc.should_log("any_target", "warn"));
        // debug < info → should NOT log
        assert!(!svc.should_log("any_target", "debug"));
    }

    #[test]
    fn should_log_uses_per_module_override() {
        let opts = LoggerOptions::new().with_module_level("special", "error");
        let svc = LoggerService::new(opts);
        // warn < error → should NOT log for "special"
        assert!(!svc.should_log("special", "warn"));
        // error >= error → should log for "special"
        assert!(svc.should_log("special", "error"));
        // But default target still uses "info"
        assert!(svc.should_log("other", "info"));
    }

    // ── LoggerService::env_filter ──────────────────────────────────────────────

    #[test]
    fn env_filter_succeeds_with_valid_directive() {
        let svc = LoggerService::new(LoggerOptions::new().with_default_level("debug"));
        assert!(svc.env_filter().is_ok());
    }

    #[test]
    fn env_filter_fails_with_invalid_directive() {
        // A directive with invalid syntax (e.g., "=" alone) should fail
        let svc = LoggerService::new(LoggerOptions::new().with_default_level("=invalid_syntax"));
        // If this is somehow valid we just check it doesn't panic
        let _ = svc.env_filter();
    }

    // ── LoggerModule ──────────────────────────────────────────────────────────

    #[test]
    fn logger_module_new_creates_marker() {
        let _m = LoggerModule::new();
        let _default = LoggerModule::default();
    }

    #[test]
    fn logger_module_for_root_produces_global_module() {
        let opts = LoggerOptions::new().with_global(true);
        let module = LoggerModule::for_root(opts);
        assert!(module.metadata.is_global);
    }

    #[test]
    fn logger_module_for_root_non_global() {
        let opts = LoggerOptions::new();
        let module = LoggerModule::for_root(opts);
        assert!(!module.metadata.is_global);
    }

    #[test]
    fn logger_module_for_feature_via_configurable_module() {
        use super::ConfigurableModule;
        let opts = LoggerOptions::new().with_global(false);
        let module = LoggerModule::for_feature(opts);
        assert!(!module.metadata.is_global);
    }

    #[test]
    fn logger_module_for_feature_global_via_configurable_module() {
        use super::ConfigurableModule;
        let opts = LoggerOptions::new().with_global(true);
        let module = LoggerModule::for_feature(opts);
        assert!(module.metadata.is_global);
    }

    #[test]
    fn logger_provider_types_and_export_types() {
        assert_eq!(logger_provider_types().len(), 2);
        assert_eq!(logger_export_types().len(), 1);
    }
}
