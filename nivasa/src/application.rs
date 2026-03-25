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
}
