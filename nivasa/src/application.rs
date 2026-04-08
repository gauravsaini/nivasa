//! Application-level configuration surfaces for the umbrella crate.
//!
//! This module intentionally stays small until `NestApplication` lands. It
//! gives the Phase 2 bootstrap work a stable place for server and versioning
//! configuration without pulling transport details into the main crate yet.

use nivasa_common::HttpException;
use nivasa_filters::{ExceptionFilter, ExceptionFilterMetadata};
use nivasa_http::{NivasaMiddleware, NivasaResponse, NivasaServer, NivasaServerBuilder};
use nivasa_guards::Guard;
use nivasa_interceptors::Interceptor;
use nivasa_pipes::Pipe;
use nivasa_routing::{RouteDispatchError, RouteMethod};

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

    /// Enable the minimal transport-side CORS bridge.
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

/// Minimal application shell for the umbrella crate.
///
/// This stays intentionally data-only until the wider application bootstrap
/// surface lands. It preserves the root module and bootstrap configuration
/// without claiming build, listen, or shutdown behavior yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NestApplication<T> {
    app_module: T,
    bootstrap: AppBootstrapConfig,
}

impl<T> NestApplication<T> {
    /// Create an application shell from the root module using default bootstrap
    /// configuration.
    pub fn create(app_module: T) -> Self {
        Self {
            app_module,
            bootstrap: AppBootstrapConfig::default(),
        }
    }

    /// Borrow the root application module.
    pub fn app_module(&self) -> &T {
        &self.app_module
    }

    /// Borrow the current bootstrap configuration.
    pub fn bootstrap(&self) -> &AppBootstrapConfig {
        &self.bootstrap
    }
}

impl AppBootstrapConfig {
    /// Create bootstrap config from server options.
    pub fn new(server: ServerOptions) -> Self {
        Self { server }
    }

    /// Expose the global route prefix for bootstrap-time route registration.
    pub fn global_prefix(&self) -> Option<&str> {
        self.server.global_prefix.as_deref()
    }

    /// Expose the configured versioning surface for bootstrap-time route setup.
    ///
    /// This stays read-only and pure so the bootstrap layer can inspect
    /// versioning choices without implying any runtime wiring beyond the
    /// existing server configuration boundary.
    pub fn versioning(&self) -> Option<&VersioningOptions> {
        self.server.versioning.as_ref()
    }

    /// Attach versioning configuration at bootstrap time.
    ///
    /// This keeps versioning as pure configuration on the bootstrap boundary
    /// and does not add any new transport/runtime dispatch behavior.
    pub fn enable_versioning(mut self, versioning: VersioningOptions) -> Self {
        self.server = self.server.with_versioning(versioning);
        self
    }

    /// Adapt app bootstrap config into the existing transport builder.
    ///
    /// This stays limited to bootstrap-owned transport flags. Route prefixing,
    /// version-aware dispatch, and the SCXML request lifecycle remain owned by
    /// the downstream routing and HTTP layers.
    pub fn server_builder(&self) -> NivasaServerBuilder {
        let mut builder = NivasaServer::builder();

        if self.server.cors {
            builder = builder.enable_cors();
        }

        builder
    }

    /// Register a bootstrap-owned unversioned route with the configured prefix.
    ///
    /// This is the smallest honest bootstrap route surface: it only prefixes
    /// the route path and delegates to the existing HTTP route builder.
    pub fn route(
        &self,
        method: impl Into<RouteMethod>,
        path: impl Into<String>,
        handler: impl Fn(&nivasa_http::NivasaRequest) -> NivasaResponse + Send + Sync + 'static,
    ) -> Result<NivasaServerBuilder, RouteDispatchError> {
        let path = path.into();

        self.server_builder()
            .route(method, self.prefixed_route_path(path.as_str()), handler)
    }

    /// Register a single global middleware at bootstrap time.
    ///
    /// This is intentionally a thin facade over the existing transport
    /// middleware hook. It does not imply module-level registration, ordering,
    /// exclusions, or decorator parsing.
    pub fn use_middleware<M>(&self, middleware: M) -> NivasaServerBuilder
    where
        M: NivasaMiddleware + Send + Sync + 'static,
    {
        self.server_builder().middleware(middleware)
    }

    /// Register a single global interceptor at bootstrap time.
    ///
    /// This remains a thin facade over the existing transport interceptor
    /// hook. It does not imply module wiring, ordering, or response mapping.
    pub fn use_interceptor<I>(&self, interceptor: I) -> NivasaServerBuilder
    where
        I: Interceptor<Response = nivasa_http::NivasaResponse> + Send + Sync + 'static,
    {
        self.server_builder().interceptor(interceptor)
    }

    /// Register a single global interceptor at bootstrap time.
    ///
    /// This is a thin alias over [`AppBootstrapConfig::use_interceptor`]
    /// so callers can use the more explicit global naming convention.
    pub fn use_global_interceptor<I>(&self, interceptor: I) -> NivasaServerBuilder
    where
        I: Interceptor<Response = nivasa_http::NivasaResponse> + Send + Sync + 'static,
    {
        self.use_interceptor(interceptor)
    }

    /// Register a single global guard at bootstrap time.
    ///
    /// This is a thin facade over the existing transport guard hook. It keeps
    /// the bootstrap layer focused on configuration and leaves runtime guard
    /// semantics to the HTTP layer.
    pub fn use_global_guard<G>(&self, guard: G) -> NivasaServerBuilder
    where
        G: Guard + Send + Sync + 'static,
    {
        self.server_builder().use_global_guard(guard)
    }

    /// Register a single global pipe at bootstrap time.
    ///
    /// This is a thin facade over the existing transport pipe hook. It keeps
    /// the bootstrap layer focused on configuration and leaves runtime pipe
    /// semantics to the HTTP layer.
    pub fn use_global_pipe<P>(&self, pipe: P) -> NivasaServerBuilder
    where
        P: Pipe + Send + Sync + 'static,
    {
        self.server_builder().use_global_pipe(pipe)
    }

    /// Register a single global exception filter at bootstrap time.
    ///
    /// This is a thin facade over the existing transport filter hook. It keeps
    /// the bootstrap layer focused on configuration and leaves runtime filter
    /// semantics to the HTTP layer.
    pub fn use_global_filter<F>(&self, filter: F) -> NivasaServerBuilder
    where
        F: ExceptionFilter<HttpException, NivasaResponse>
            + ExceptionFilterMetadata
            + Send
            + Sync
            + 'static,
    {
        self.server_builder().use_global_filter(filter)
    }

    /// Compose a bootstrap-time route path from the configured global prefix.
    ///
    /// This stays as pure string handling so route registration can consume it
    /// later without implying that runtime wiring already exists.
    pub fn prefixed_route_path(&self, path: impl AsRef<str>) -> String {
        let route = normalize_route_path(path.as_ref());

        match self.global_prefix() {
            Some(prefix) if route == "/" => prefix.to_string(),
            Some(prefix) => format!("{}{}", prefix, route),
            None => route,
        }
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

    /// Enable the minimal transport-side CORS bridge.
    pub fn enable_cors(mut self) -> Self {
        self.cors = true;
        self
    }

    /// Toggle the minimal transport-side CORS bridge explicitly.
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

fn normalize_route_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return String::from("/");
    }

    let stripped = trimmed.trim_start_matches('/').trim_end_matches('/');
    format!("/{}", stripped)
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
    use nivasa_http::{NextMiddleware, NivasaRequest, NivasaResponse};
    use nivasa_routing::RouteMethod;

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
            .versioning(VersioningOptions::builder(VersioningStrategy::Header).build())
            .global_prefix("api")
            .build();
        let bootstrap = AppBootstrapConfig::from(server.clone());

        assert_eq!(bootstrap.server, server);
        assert_eq!(
            bootstrap.versioning().map(|options| options.strategy),
            Some(VersioningStrategy::Header)
        );
        assert_eq!(
            AppBootstrapConfig::default().server,
            ServerOptions::default()
        );
        assert_eq!(AppBootstrapConfig::default().versioning(), None);
    }

    #[test]
    fn bootstrap_config_prefixes_route_paths_purely_for_future_bootstrap_use() {
        let bootstrap =
            AppBootstrapConfig::from(ServerOptions::builder().global_prefix(" api/ ").build());

        assert_eq!(bootstrap.prefixed_route_path("users"), "/api/users");
        assert_eq!(bootstrap.prefixed_route_path("/users/"), "/api/users");
        assert_eq!(bootstrap.prefixed_route_path("/"), "/api");
        assert_eq!(
            AppBootstrapConfig::default().prefixed_route_path(" users/ "),
            "/users"
        );
    }

    #[test]
    fn bootstrap_config_adapts_into_the_existing_server_builder() {
        let bootstrap = AppBootstrapConfig::from(ServerOptions::builder().enable_cors().build());
        let builder = bootstrap
            .server_builder()
            .route(RouteMethod::Get, "/health", |_| NivasaResponse::text("ok"))
            .expect("route registration should succeed");

        let _server = builder.build();
    }

    #[test]
    fn bootstrap_config_prefixes_unversioned_routes_during_registration() {
        let bootstrap =
            AppBootstrapConfig::from(ServerOptions::builder().global_prefix(" api/ ").build());
        let builder = bootstrap
            .route(RouteMethod::Get, "health", |_| NivasaResponse::text("ok"))
            .expect("prefixed route registration should succeed");

        assert_eq!(bootstrap.prefixed_route_path("health"), "/api/health");
        let _server = builder.build();
    }

    #[test]
    fn bootstrap_config_can_forward_global_middleware_into_transport_builder() {
        let bootstrap = AppBootstrapConfig::default();
        let builder = bootstrap
            .use_middleware(|request: NivasaRequest, next: NextMiddleware| async move {
                next.run(request).await
            })
            .route(RouteMethod::Get, "/health", |_| NivasaResponse::text("ok"))
            .expect("route registration should succeed");

        let _server = builder.build();
    }
}
