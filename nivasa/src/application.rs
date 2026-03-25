//! Application-level configuration surfaces for the umbrella crate.
//!
//! This module intentionally stays small until `NestApplication` lands. It
//! gives the Phase 2 bootstrap work a stable place for server and versioning
//! configuration without pulling transport details into the main crate yet.

/// Supported API versioning strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VersioningStrategy {
    /// Versioned URIs such as `/v1/users`.
    #[default]
    Uri,
    /// Version selection via the `X-API-Version` request header.
    Header,
    /// Version selection via an `Accept` media type.
    MediaType,
}

/// App-level versioning configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersioningOptions {
    pub strategy: VersioningStrategy,
    pub default_version: Option<String>,
}

impl VersioningOptions {
    /// Start building a versioning configuration.
    pub fn builder(strategy: VersioningStrategy) -> VersioningOptionsBuilder {
        VersioningOptionsBuilder::new(strategy)
    }

    /// Create a new versioning configuration for the given strategy.
    pub fn new(strategy: VersioningStrategy) -> Self {
        Self {
            strategy,
            default_version: None,
        }
    }

    /// Set the default API version.
    pub fn with_default_version(mut self, version: impl Into<String>) -> Self {
        let version = version.into();
        self.default_version = if version.trim().is_empty() {
            None
        } else {
            Some(normalize_version_token(&version))
        };
        self
    }
}

impl Default for VersioningOptions {
    fn default() -> Self {
        Self::new(VersioningStrategy::default())
    }
}

/// Fluent builder for [`VersioningOptions`].
#[derive(Debug, Clone)]
pub struct VersioningOptionsBuilder {
    strategy: VersioningStrategy,
    default_version: Option<String>,
}

impl VersioningOptionsBuilder {
    fn new(strategy: VersioningStrategy) -> Self {
        Self {
            strategy,
            default_version: None,
        }
    }

    /// Set the default API version.
    pub fn default_version(mut self, version: impl Into<String>) -> Self {
        let version = version.into();
        self.default_version = if version.trim().is_empty() {
            None
        } else {
            Some(normalize_version_token(&version))
        };
        self
    }

    /// Finish constructing the versioning configuration.
    pub fn build(self) -> VersioningOptions {
        VersioningOptions {
            strategy: self.strategy,
            default_version: self.default_version,
        }
    }
}

impl From<VersioningOptionsBuilder> for VersioningOptions {
    fn from(builder: VersioningOptionsBuilder) -> Self {
        builder.build()
    }
}

/// Transport-facing server options owned by the umbrella crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerOptions {
    pub host: String,
    pub port: u16,
    pub cors: bool,
    pub global_prefix: Option<String>,
    pub versioning: Option<VersioningOptions>,
}

impl ServerOptions {
    /// Start building server options.
    pub fn builder() -> ServerOptionsBuilder {
        ServerOptionsBuilder::default()
    }

    /// Create server options with the provided host and port.
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            cors: false,
            global_prefix: None,
            versioning: None,
        }
    }

    /// Attach a global route prefix such as `/api`.
    pub fn with_global_prefix(mut self, prefix: impl Into<String>) -> Self {
        let prefix = normalize_path_prefix(prefix.into());
        self.global_prefix = if prefix.is_empty() {
            None
        } else {
            Some(prefix)
        };
        self
    }

    /// Enable CORS for the eventual server integration.
    pub fn enable_cors(mut self) -> Self {
        self.cors = true;
        self
    }

    /// Attach versioning config to the server surface.
    pub fn with_versioning(mut self, versioning: VersioningOptions) -> Self {
        self.versioning = Some(versioning);
        self
    }
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self::new("127.0.0.1", 3000)
    }
}

/// App-only bootstrap boundary that stays as pure configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppBootstrapConfig {
    pub server: ServerOptions,
}

impl AppBootstrapConfig {
    /// Create bootstrap config from server options.
    pub fn new(server: ServerOptions) -> Self {
        Self { server }
    }
}

impl Default for AppBootstrapConfig {
    fn default() -> Self {
        Self::new(ServerOptions::default())
    }
}

impl From<ServerOptions> for AppBootstrapConfig {
    fn from(server: ServerOptions) -> Self {
        Self::new(server)
    }
}

/// Fluent builder for [`ServerOptions`].
#[derive(Debug, Clone)]
pub struct ServerOptionsBuilder {
    host: String,
    port: u16,
    cors: bool,
    global_prefix: Option<String>,
    versioning: Option<VersioningOptions>,
}

impl ServerOptionsBuilder {
    /// Override the listen host.
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    /// Override the listen port.
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Enable CORS for the eventual server integration.
    pub fn enable_cors(mut self) -> Self {
        self.cors = true;
        self
    }

    /// Toggle CORS explicitly.
    pub fn cors(mut self, cors: bool) -> Self {
        self.cors = cors;
        self
    }

    /// Attach a global route prefix such as `/api`.
    pub fn global_prefix(mut self, prefix: impl Into<String>) -> Self {
        let prefix = normalize_path_prefix(prefix.into());
        self.global_prefix = if prefix.is_empty() {
            None
        } else {
            Some(prefix)
        };
        self
    }

    /// Attach versioning config to the server surface.
    pub fn versioning(mut self, versioning: VersioningOptions) -> Self {
        self.versioning = Some(versioning);
        self
    }

    /// Finish constructing the server options.
    pub fn build(self) -> ServerOptions {
        ServerOptions {
            host: self.host,
            port: self.port,
            cors: self.cors,
            global_prefix: self.global_prefix,
            versioning: self.versioning,
        }
    }
}

impl Default for ServerOptionsBuilder {
    fn default() -> Self {
        let defaults = ServerOptions::default();
        Self {
            host: defaults.host,
            port: defaults.port,
            cors: defaults.cors,
            global_prefix: defaults.global_prefix,
            versioning: defaults.versioning,
        }
    }
}

impl From<ServerOptionsBuilder> for ServerOptions {
    fn from(builder: ServerOptionsBuilder) -> Self {
        builder.build()
    }
}

fn normalize_path_prefix(prefix: String) -> String {
    let trimmed = prefix.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let prefixed = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{}", trimmed)
    };

    if prefixed.len() > 1 {
        prefixed.trim_end_matches('/').to_string()
    } else {
        prefixed
    }
}

fn normalize_version_token(version: &str) -> String {
    let trimmed = version.trim().trim_matches('/');
    let stripped = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'))
        .unwrap_or(trimmed);

    format!("v{}", stripped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_server_options_are_sane() {
        let options = ServerOptions::default();

        assert_eq!(options.host, "127.0.0.1");
        assert_eq!(options.port, 3000);
        assert!(!options.cors);
        assert_eq!(options.global_prefix, None);
        assert_eq!(options.versioning, None);
    }

    #[test]
    fn server_options_normalize_prefixes() {
        let options = ServerOptions::new("0.0.0.0", 8080).with_global_prefix("api/");

        assert_eq!(options.global_prefix.as_deref(), Some("/api"));
    }

    #[test]
    fn versioning_options_normalize_versions() {
        let options =
            VersioningOptions::new(VersioningStrategy::Header).with_default_version(" 1 ");

        assert_eq!(options.strategy, VersioningStrategy::Header);
        assert_eq!(options.default_version.as_deref(), Some("v1"));
    }

    #[test]
    fn builders_construct_the_same_config_surface() {
        let versioning = VersioningOptions::builder(VersioningStrategy::MediaType)
            .default_version(" /v3/ ")
            .build();
        let options = ServerOptions::builder()
            .host("0.0.0.0")
            .port(8080)
            .enable_cors()
            .global_prefix(" api/ ")
            .versioning(versioning.clone())
            .build();

        assert_eq!(versioning.strategy, VersioningStrategy::MediaType);
        assert_eq!(versioning.default_version.as_deref(), Some("v3"));
        assert_eq!(options.host, "0.0.0.0");
        assert_eq!(options.port, 8080);
        assert!(options.cors);
        assert_eq!(options.global_prefix.as_deref(), Some("/api"));
        assert_eq!(options.versioning, Some(versioning));
    }

    #[test]
    fn bootstrap_config_wraps_the_server_surface_without_runtime_behavior() {
        let server = ServerOptions::builder()
            .host("0.0.0.0")
            .port(8080)
            .global_prefix("api")
            .build();
        let bootstrap = AppBootstrapConfig::from(server.clone());

        assert_eq!(bootstrap.server, server);
        assert_eq!(AppBootstrapConfig::default().server, ServerOptions::default());
    }
}
