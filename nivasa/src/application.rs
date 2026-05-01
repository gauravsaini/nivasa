//! Application-level configuration surfaces for the umbrella crate.
//!
//! This module intentionally stays small until `NestApplication` lands. It
//! gives the Phase 2 bootstrap work a stable place for server and versioning
//! configuration without pulling transport details into the main crate yet.

use crate::openapi::{
    swagger_ui_index_html, OpenApiComponents, OpenApiDocument, OpenApiMediaType, OpenApiOperation,
    OpenApiParameter, OpenApiRequestBody, OpenApiResponse, OpenApiSecurityRequirement,
};
use nivasa_common::HttpException;
use nivasa_core::{
    module::{ModuleControllerRegistration, RouteThrottleRegistration},
    DependencyContainer, DiError, Module, ModuleMetadata, OnApplicationShutdown,
};
use nivasa_filters::{ExceptionFilter, ExceptionFilterMetadata};
use nivasa_guards::Guard;
use nivasa_http::{NivasaMiddleware, NivasaResponse, NivasaServer, NivasaServerBuilder};
use nivasa_interceptors::Interceptor;
use nivasa_pipes::Pipe;
use nivasa_routing::{RouteDispatchError, RouteMethod};
use serde_json::{Map, Value};
use std::any::type_name;
use std::collections::HashSet;
use std::future::Future;
use std::io;
use std::sync::Arc;

const DEFAULT_OPENAPI_SPEC_PATH: &str = "/api/docs/openapi.json";
const DEFAULT_SWAGGER_UI_PATH: &str = "/api/docs";

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

    /// Return the listen address in host:port form for startup reporting.
    pub fn listen_address(&self) -> String {
        format_listen_address(&self.host, self.port)
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
    openapi_spec_path: String,
    swagger_ui_path: String,
}

/// Minimal application shell for the umbrella crate.
///
/// The shell can carry an explicit preflight hook so startup validation can
/// fail fast before module configure or any SCXML-backed lifecycle work.
pub struct NestApplication<T> {
    app_module: T,
    bootstrap: AppBootstrapConfig,
    preflight: Option<AppPreflightHook<T>>,
}

/// A built application shell that has resolved bootstrap-owned metadata.
pub struct App<T> {
    app_module: T,
    bootstrap: AppBootstrapConfig,
    container: DependencyContainer,
    module_metadata: ModuleMetadata,
    controller_registrations: Vec<ModuleControllerRegistration>,
    routes: Vec<AppRoute>,
}

/// One route resolved during the synchronous application build step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppRoute {
    pub method: RouteMethod,
    pub path: String,
    pub handler: &'static str,
    pub throttle: Option<RouteThrottleRegistration>,
    pub skip_throttle: bool,
}

/// Startup reporting data for the root app shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppStartupReport {
    pub banner: String,
    pub root_module: &'static str,
    pub routes_registered: usize,
    pub listen_address: String,
}

impl AppStartupReport {
    /// Return the report as display-ready startup log lines.
    pub fn lines(&self) -> Vec<String> {
        vec![
            self.banner.clone(),
            format!("root module loaded: {}", self.root_module),
            format!("routes registered: {}", self.routes_registered),
            format!("listen address: {}", self.listen_address),
        ]
    }
}

/// Errors raised while assembling the minimal application shell.
#[derive(Debug)]
pub enum AppBuildError {
    PreflightValidation { message: String },
    DependencyInjection(DiError),
    DuplicateRoute { method: String, path: String },
    MissingRouteHandler { handler: String },
    Listen(io::Error),
}

impl std::fmt::Display for AppBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PreflightValidation { message } => {
                write!(f, "preflight validation failed: {message}")
            }
            Self::DependencyInjection(error) => write!(f, "{error}"),
            Self::DuplicateRoute { method, path } => {
                write!(f, "duplicate route `{method} {path}` while building app")
            }
            Self::MissingRouteHandler { handler } => {
                write!(
                    f,
                    "missing route handler `{handler}` while building app server"
                )
            }
            Self::Listen(error) => write!(f, "listen error: {error}"),
        }
    }
}

impl std::error::Error for AppBuildError {}

impl From<DiError> for AppBuildError {
    fn from(value: DiError) -> Self {
        Self::DependencyInjection(value)
    }
}

impl From<io::Error> for AppBuildError {
    fn from(value: io::Error) -> Self {
        Self::Listen(value)
    }
}

impl<T> NestApplication<T> {
    /// Create an application shell from the root module using default bootstrap
    /// configuration.
    pub fn create(app_module: T) -> Self {
        Self {
            app_module,
            bootstrap: AppBootstrapConfig::default(),
            preflight: None,
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

    /// Attach an explicit startup preflight gate.
    ///
    /// The hook runs before module configure and before any SCXML-backed
    /// lifecycle work. It is meant for fail-fast validation such as config
    /// required-key checks.
    pub fn with_preflight<F>(mut self, preflight: F) -> Self
    where
        F: Fn(&T, &AppBootstrapConfig) -> Result<(), AppBuildError> + Send + Sync + 'static,
    {
        self.preflight = Some(Box::new(preflight));
        self
    }
}

impl<T: Module> NestApplication<T> {
    /// Build an application shell by resolving the root module's metadata,
    /// dependency container registrations, and controller route metadata.
    pub fn build(self) -> Result<App<T>, AppBuildError> {
        if let Some(preflight) = self.preflight.as_ref() {
            preflight(&self.app_module, &self.bootstrap)?;
        }

        let module_metadata = self.app_module.metadata();
        let controller_registrations = self.app_module.controller_registrations();
        let container = DependencyContainer::new();

        block_on(self.app_module.configure(&container))?;
        block_on(container.initialize())?;

        let routes = resolve_routes(&self.bootstrap, &controller_registrations)?;

        Ok(App {
            app_module: self.app_module,
            bootstrap: self.bootstrap,
            container,
            module_metadata,
            controller_registrations,
            routes,
        })
    }

    /// Build the application and start the HTTP server on the provided options.
    pub async fn listen(self, server: ServerOptions) -> Result<(), AppBuildError> {
        let Self {
            app_module,
            preflight,
            ..
        } = self;

        let app = Self {
            app_module,
            bootstrap: AppBootstrapConfig::from(server),
            preflight,
        };

        app.build()?.listen().await
    }
}

impl<T> App<T> {
    /// Borrow the root application module stored in the built app.
    pub fn app_module(&self) -> &T {
        &self.app_module
    }

    /// Borrow the bootstrap configuration captured during build.
    pub fn bootstrap(&self) -> &AppBootstrapConfig {
        &self.bootstrap
    }

    /// Borrow the initialized dependency container.
    pub fn container(&self) -> &DependencyContainer {
        &self.container
    }

    /// Borrow the root module metadata captured during build.
    pub fn module_metadata(&self) -> &ModuleMetadata {
        &self.module_metadata
    }

    /// Borrow the root module controller registrations captured during build.
    pub fn controller_registrations(&self) -> &[ModuleControllerRegistration] {
        &self.controller_registrations
    }

    /// Borrow the resolved route metadata captured during build.
    pub fn routes(&self) -> &[AppRoute] {
        &self.routes
    }

    /// Build the startup report for banner and logging surfaces.
    pub fn startup_report(&self) -> AppStartupReport {
        AppStartupReport {
            banner: startup_banner(),
            root_module: type_name::<T>(),
            routes_registered: self.routes.len(),
            listen_address: self.bootstrap.listen_address(),
        }
    }

    /// Return the startup report as display-ready log lines.
    pub fn startup_lines(&self) -> Vec<String> {
        self.startup_report().lines()
    }

    /// Run application shutdown hooks for testing or controlled teardown.
    ///
    /// This is only available when the root module actually exposes an
    /// application shutdown hook. The app shell consumes itself and then
    /// runs the root module shutdown callback.
    pub fn close(self) -> Result<(), AppBuildError>
    where
        T: OnApplicationShutdown,
    {
        block_on(self.app_module.on_application_shutdown());
        Ok(())
    }

    /// Build an in-memory server from the resolved app routes.
    ///
    /// The adapter stays honest: routes come from the built app metadata, and
    /// the caller supplies the actual route handlers by name.
    pub fn to_server<F>(&self, resolve_handler: F) -> Result<NivasaServer, AppBuildError>
    where
        F: Fn(&AppRoute) -> Option<AppRouteHandler> + Send + Sync + 'static,
    {
        let mut builder = self.bootstrap.server_builder();

        for route in &self.routes {
            let Some(handler) = resolve_handler(route) else {
                return Err(AppBuildError::MissingRouteHandler {
                    handler: route.handler.to_string(),
                });
            };

            let handler = Arc::clone(&handler);
            let method = route.method.clone();
            let path = route.path.clone();
            builder = if route.skip_throttle {
                builder
                    .route(method, path, move |request| (handler)(request))
                    .map_err(|error| match error {
                        RouteDispatchError::DuplicateRoute { method, path } => {
                            AppBuildError::DuplicateRoute { method, path }
                        }
                        RouteDispatchError::UnsupportedPatternSegment { path, .. } => {
                            AppBuildError::DuplicateRoute {
                                method: route.method.as_str().to_string(),
                                path,
                            }
                        }
                    })?
            } else if let Some(throttle) = route.throttle.clone() {
                builder
                    .route_with_throttle(method, path, move |request| (handler)(request), throttle)
                    .map_err(|error| match error {
                        RouteDispatchError::DuplicateRoute { method, path } => {
                            AppBuildError::DuplicateRoute { method, path }
                        }
                        RouteDispatchError::UnsupportedPatternSegment { path, .. } => {
                            AppBuildError::DuplicateRoute {
                                method: route.method.as_str().to_string(),
                                path,
                            }
                        }
                    })?
            } else {
                builder
                    .route(method, path, move |request| (handler)(request))
                    .map_err(|error| match error {
                        RouteDispatchError::DuplicateRoute { method, path } => {
                            AppBuildError::DuplicateRoute { method, path }
                        }
                        RouteDispatchError::UnsupportedPatternSegment { path, .. } => {
                            AppBuildError::DuplicateRoute {
                                method: route.method.as_str().to_string(),
                                path,
                            }
                        }
                    })?
            };
        }

        Ok(builder.build())
    }

    /// Start the HTTP server using controller handlers registered by macros.
    pub async fn listen(self) -> Result<(), AppBuildError> {
        let host = self.bootstrap.server.host.clone();
        let port = self.bootstrap.server.port;
        let global_prefix = self.bootstrap.global_prefix().map(str::to_string);

        let server = self.to_server(move |route| {
            let lookup_path = controller_lookup_path(global_prefix.as_deref(), route.path.as_str());
            nivasa_http::resolve_controller_route_handler(&lookup_path, route.handler)
        })?;

        server.listen(host, port).await.map_err(AppBuildError::from)
    }
}

impl AppBootstrapConfig {
    /// Create bootstrap config from server options.
    pub fn new(server: ServerOptions) -> Self {
        Self {
            server,
            openapi_spec_path: DEFAULT_OPENAPI_SPEC_PATH.to_string(),
            swagger_ui_path: DEFAULT_SWAGGER_UI_PATH.to_string(),
        }
    }

    /// Expose the global route prefix for bootstrap-time route registration.
    pub fn global_prefix(&self) -> Option<&str> {
        self.server.global_prefix.as_deref()
    }

    /// Return the configured listen address for startup reporting.
    pub fn listen_address(&self) -> String {
        self.server.listen_address()
    }

    /// Expose the configured versioning surface for bootstrap-time route setup.
    ///
    /// This stays read-only and pure so the bootstrap layer can inspect
    /// versioning choices without implying any runtime wiring beyond the
    /// existing server configuration boundary.
    pub fn versioning(&self) -> Option<&VersioningOptions> {
        self.server.versioning.as_ref()
    }

    /// Path where the OpenAPI JSON document is served.
    pub fn openapi_spec_path(&self) -> &str {
        &self.openapi_spec_path
    }

    /// Path where the Swagger UI shell is served.
    pub fn swagger_ui_path(&self) -> &str {
        &self.swagger_ui_path
    }

    /// Override OpenAPI JSON path.
    pub fn with_openapi_spec_path(mut self, path: impl Into<String>) -> Self {
        let path = path.into().trim().to_string();
        self.openapi_spec_path = if path.is_empty() {
            DEFAULT_OPENAPI_SPEC_PATH.to_string()
        } else if path.starts_with('/') {
            path
        } else {
            format!("/{path}")
        };
        self
    }

    /// Override Swagger UI mount path.
    pub fn with_swagger_ui_path(mut self, path: impl Into<String>) -> Self {
        let path = normalize_swagger_ui_path(path.into());
        self.swagger_ui_path = if path.is_empty() {
            DEFAULT_SWAGGER_UI_PATH.to_string()
        } else {
            path
        };
        self
    }

    /// Register an OpenAPI JSON endpoint using the configured path.
    pub fn serve_openapi_spec(
        &self,
        document: &OpenApiDocument,
    ) -> Result<NivasaServerBuilder, RouteDispatchError> {
        self.server_builder().openapi_spec_json(
            self.openapi_spec_path.clone(),
            openapi_document_to_value(document),
        )
    }

    /// Register the Swagger UI shell at the configured mount path.
    pub fn serve_swagger_ui(&self) -> Result<NivasaServerBuilder, RouteDispatchError> {
        let html = swagger_ui_index_html(
            self.openapi_spec_path.clone(),
            "Nivasa API Docs",
            "OpenAPI documentation",
            "1.0.0",
        );

        self.server_builder()
            .route(RouteMethod::Get, self.swagger_ui_path.clone(), move |_| {
                NivasaResponse::html(html.clone())
            })
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

fn openapi_document_to_value(document: &OpenApiDocument) -> Value {
    Value::Object(Map::from_iter([
        (
            "openapi".to_string(),
            Value::String(document.openapi.clone()),
        ),
        (
            "info".to_string(),
            Value::Object(Map::from_iter([
                (
                    "title".to_string(),
                    Value::String(document.info.title.clone()),
                ),
                (
                    "version".to_string(),
                    Value::String(document.info.version.clone()),
                ),
            ])),
        ),
        (
            "paths".to_string(),
            Value::Object(openapi_paths_to_value(&document.paths)),
        ),
        (
            "components".to_string(),
            Value::Object(openapi_components_to_value(&document.components)),
        ),
    ]))
}

fn openapi_paths_to_value(
    paths: &std::collections::BTreeMap<
        String,
        std::collections::BTreeMap<String, OpenApiOperation>,
    >,
) -> Map<String, Value> {
    paths
        .iter()
        .map(|(path, operations)| {
            (
                path.clone(),
                Value::Object(
                    operations
                        .iter()
                        .map(|(method, operation)| {
                            (method.clone(), openapi_operation_to_value(operation))
                        })
                        .collect(),
                ),
            )
        })
        .collect()
}

fn openapi_operation_to_value(operation: &OpenApiOperation) -> Value {
    Value::Object(Map::from_iter([
        (
            "tags".to_string(),
            Value::Array(operation.tags.iter().cloned().map(Value::String).collect()),
        ),
        (
            "summary".to_string(),
            operation
                .summary
                .as_ref()
                .map(|summary| Value::String(summary.clone()))
                .unwrap_or(Value::Null),
        ),
        (
            "parameters".to_string(),
            Value::Array(
                operation
                    .parameters
                    .iter()
                    .map(openapi_parameter_to_value)
                    .collect(),
            ),
        ),
        (
            "requestBody".to_string(),
            operation
                .request_body
                .as_ref()
                .map(openapi_request_body_to_value)
                .unwrap_or(Value::Null),
        ),
        (
            "responses".to_string(),
            Value::Object(
                operation
                    .responses
                    .iter()
                    .map(|(status, response)| (status.clone(), openapi_response_to_value(response)))
                    .collect(),
            ),
        ),
        (
            "security".to_string(),
            Value::Array(
                operation
                    .security
                    .iter()
                    .map(openapi_security_requirement_to_value)
                    .collect(),
            ),
        ),
    ]))
}

fn openapi_parameter_to_value(parameter: &OpenApiParameter) -> Value {
    Value::Object(Map::from_iter([
        ("name".to_string(), Value::String(parameter.name.clone())),
        ("in".to_string(), Value::String(parameter.location.clone())),
        (
            "description".to_string(),
            Value::String(parameter.description.clone()),
        ),
        ("required".to_string(), Value::Bool(parameter.required)),
        (
            "schema".to_string(),
            Value::Object(Map::from_iter([(
                "type".to_string(),
                Value::String(parameter.schema.schema_type.clone()),
            )])),
        ),
    ]))
}

fn openapi_request_body_to_value(request_body: &OpenApiRequestBody) -> Value {
    Value::Object(Map::from_iter([
        ("required".to_string(), Value::Bool(request_body.required)),
        (
            "content".to_string(),
            Value::Object(
                request_body
                    .content
                    .iter()
                    .map(|(content_type, media_type)| {
                        (
                            content_type.clone(),
                            openapi_media_type_to_value(media_type),
                        )
                    })
                    .collect(),
            ),
        ),
    ]))
}

fn openapi_response_to_value(response: &OpenApiResponse) -> Value {
    Value::Object(Map::from_iter([
        (
            "description".to_string(),
            Value::String(response.description.clone()),
        ),
        (
            "content".to_string(),
            Value::Object(
                response
                    .content
                    .iter()
                    .map(|(content_type, media_type)| {
                        (
                            content_type.clone(),
                            openapi_media_type_to_value(media_type),
                        )
                    })
                    .collect(),
            ),
        ),
    ]))
}

fn openapi_media_type_to_value(media_type: &OpenApiMediaType) -> Value {
    Value::Object(Map::from_iter([(
        "schema".to_string(),
        Value::Object(Map::from_iter([(
            "$ref".to_string(),
            Value::String(media_type.schema_ref.clone()),
        )])),
    )]))
}

fn openapi_security_requirement_to_value(requirement: &OpenApiSecurityRequirement) -> Value {
    Value::Object(
        requirement
            .iter()
            .map(|(name, scopes)| {
                (
                    name.clone(),
                    Value::Array(scopes.iter().cloned().map(Value::String).collect()),
                )
            })
            .collect(),
    )
}

fn openapi_components_to_value(components: &OpenApiComponents) -> Map<String, Value> {
    Map::from_iter([
        (
            "schemas".to_string(),
            Value::Object(
                components
                    .schemas
                    .iter()
                    .map(|(name, schema)| {
                        (
                            name.clone(),
                            Value::Object(Map::from_iter([(
                                "type".to_string(),
                                Value::String(schema.schema_type.clone()),
                            )])),
                        )
                    })
                    .collect(),
            ),
        ),
        (
            "securitySchemes".to_string(),
            Value::Object(
                components
                    .security_schemes
                    .iter()
                    .map(|(name, scheme)| {
                        (
                            name.clone(),
                            Value::Object(Map::from_iter([
                                (
                                    "type".to_string(),
                                    Value::String(scheme.scheme_type.clone()),
                                ),
                                ("scheme".to_string(), Value::String(scheme.scheme.clone())),
                                (
                                    "bearerFormat".to_string(),
                                    scheme
                                        .bearer_format
                                        .as_ref()
                                        .map(|value| Value::String(value.clone()))
                                        .unwrap_or(Value::Null),
                                ),
                            ])),
                        )
                    })
                    .collect(),
            ),
        ),
    ])
}

fn normalize_swagger_ui_path(path: String) -> String {
    let path = path.trim().to_string();

    if path.is_empty() {
        String::new()
    } else if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    }
}

#[cfg(test)]
mod openapi_tests {
    use super::*;
    use crate::openapi::{
        build_openapi_document, OpenApiControllerMetadata, OpenApiControllerMetadataProvider,
    };

    struct DocsController;

    impl OpenApiControllerMetadataProvider for DocsController {
        fn routes() -> Vec<(&'static str, String, &'static str)> {
            vec![("GET", "/docs/:id".to_string(), "show")]
        }

        fn api_tags() -> Vec<&'static str> {
            vec!["Docs"]
        }

        fn api_operation_metadata() -> Vec<(&'static str, Option<&'static str>)> {
            vec![("show", Some("Show docs"))]
        }

        fn api_param_metadata() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
            vec![("show", vec![("id", "Doc ID")])]
        }
    }

    #[test]
    fn openapi_spec_path_defaults_and_overrides() {
        let bootstrap = AppBootstrapConfig::default();
        assert_eq!(bootstrap.openapi_spec_path(), DEFAULT_OPENAPI_SPEC_PATH);
        assert_eq!(bootstrap.swagger_ui_path(), DEFAULT_SWAGGER_UI_PATH);

        let custom = bootstrap
            .clone()
            .with_openapi_spec_path("custom/openapi.json");
        assert_eq!(custom.openapi_spec_path(), "/custom/openapi.json");

        let custom_ui = bootstrap.with_swagger_ui_path("custom/docs");
        assert_eq!(custom_ui.swagger_ui_path(), "/custom/docs");
    }

    #[test]
    fn swagger_ui_mount_uses_configured_path_and_openapi_spec() {
        let bootstrap = AppBootstrapConfig::default()
            .with_openapi_spec_path("/docs/openapi.json")
            .with_swagger_ui_path("docs");

        assert_eq!(bootstrap.swagger_ui_path(), "/docs");
        assert!(bootstrap.serve_swagger_ui().is_ok());
    }

    #[test]
    fn openapi_document_bridge_preserves_core_shape() {
        let document = build_openapi_document(
            "Docs",
            "1.0.0",
            [OpenApiControllerMetadata::from_provider::<DocsController>()],
        );

        let value = openapi_document_to_value(&document);

        assert_eq!(value["openapi"], "3.0.0");
        assert_eq!(value["info"]["title"], "Docs");
        assert_eq!(value["paths"]["/docs/{id}"]["get"]["summary"], "Show docs");
        assert_eq!(
            value["paths"]["/docs/{id}"]["get"]["parameters"][0]["name"],
            "id"
        );
        assert_eq!(
            value["paths"]["/docs/{id}"]["get"]["parameters"][0]["schema"]["type"],
            "string"
        );
    }

    #[test]
    fn openapi_document_bridge_preserves_full_operation_shape() {
        use std::collections::BTreeMap;

        let mut security = BTreeMap::new();
        security.insert("bearerAuth".to_string(), vec!["read:docs".to_string()]);

        let document = OpenApiDocument {
            openapi: "3.1.0".to_string(),
            info: crate::openapi::OpenApiInfo {
                title: "Admin".to_string(),
                version: "2.0.0".to_string(),
            },
            paths: BTreeMap::from([(
                "/admin".to_string(),
                BTreeMap::from([(
                    "post".to_string(),
                    OpenApiOperation {
                        tags: vec!["Admin".to_string()],
                        summary: None,
                        parameters: vec![OpenApiParameter {
                            name: "tenant".to_string(),
                            location: "header".to_string(),
                            description: "Tenant id".to_string(),
                            required: false,
                            schema: crate::openapi::OpenApiInlineSchema {
                                schema_type: "uuid".to_string(),
                            },
                        }],
                        request_body: Some(OpenApiRequestBody {
                            required: true,
                            content: BTreeMap::from([(
                                "application/json".to_string(),
                                OpenApiMediaType {
                                    schema_ref: "#/components/schemas/CreateAdmin".to_string(),
                                },
                            )]),
                        }),
                        responses: BTreeMap::from([(
                            "201".to_string(),
                            OpenApiResponse {
                                description: "created".to_string(),
                                content: BTreeMap::from([(
                                    "application/json".to_string(),
                                    OpenApiMediaType {
                                        schema_ref: "#/components/schemas/Admin".to_string(),
                                    },
                                )]),
                            },
                        )]),
                        security: vec![security],
                    },
                )]),
            )]),
            components: OpenApiComponents {
                schemas: BTreeMap::from([(
                    "Admin".to_string(),
                    crate::openapi::OpenApiSchema {
                        schema_type: "object".to_string(),
                    },
                )]),
                security_schemes: BTreeMap::from([
                    (
                        "bearerAuth".to_string(),
                        crate::openapi::OpenApiSecurityScheme {
                            scheme_type: "http".to_string(),
                            scheme: "bearer".to_string(),
                            bearer_format: Some("JWT".to_string()),
                        },
                    ),
                    (
                        "apiKey".to_string(),
                        crate::openapi::OpenApiSecurityScheme {
                            scheme_type: "apiKey".to_string(),
                            scheme: "header".to_string(),
                            bearer_format: None,
                        },
                    ),
                ]),
            },
        };

        let value = openapi_document_to_value(&document);

        assert_eq!(value["openapi"], "3.1.0");
        assert_eq!(value["paths"]["/admin"]["post"]["summary"], Value::Null);
        assert_eq!(
            value["paths"]["/admin"]["post"]["requestBody"]["content"]["application/json"]
                ["schema"]["$ref"],
            "#/components/schemas/CreateAdmin"
        );
        assert_eq!(
            value["paths"]["/admin"]["post"]["responses"]["201"]["content"]["application/json"]
                ["schema"]["$ref"],
            "#/components/schemas/Admin"
        );
        assert_eq!(
            value["paths"]["/admin"]["post"]["security"][0]["bearerAuth"][0],
            "read:docs"
        );
        assert_eq!(
            value["components"]["securitySchemes"]["bearerAuth"]["bearerFormat"],
            "JWT"
        );
        assert_eq!(
            value["components"]["securitySchemes"]["apiKey"]["bearerFormat"],
            Value::Null
        );
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

fn startup_banner() -> String {
    format!(
        r#" _   _ _ _
| \ | (_) |__   ___  ___  ___
|  \| | | '_ \ / _ \/ __|/ _ \
| |\  | | | | |  __/\__ \  __/
|_| \_|_|_| |_|\___||___/\___|
Nivasa v{}"#,
        env!("CARGO_PKG_VERSION")
    )
}

fn format_listen_address(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn resolve_routes(
    bootstrap: &AppBootstrapConfig,
    controller_registrations: &[ModuleControllerRegistration],
) -> Result<Vec<AppRoute>, AppBuildError> {
    let mut seen = HashSet::new();
    let mut routes = Vec::new();

    for controller in controller_registrations {
        for route in &controller.routes {
            let method = RouteMethod::from(route.method);
            let path = bootstrap.prefixed_route_path(route.path.as_str());

            if !seen.insert((method.clone(), path.clone())) {
                return Err(AppBuildError::DuplicateRoute {
                    method: route.method.to_string(),
                    path,
                });
            }

            routes.push(AppRoute {
                method,
                path,
                handler: route.handler,
                throttle: route.throttle.clone(),
                skip_throttle: route.skip_throttle,
            });
        }
    }

    Ok(routes)
}

fn controller_lookup_path(global_prefix: Option<&str>, path: &str) -> String {
    let path = normalize_route_path(path);

    let Some(prefix) = global_prefix else {
        return path;
    };

    let prefix = normalize_route_path(prefix);
    if prefix == "/" {
        return path;
    }

    if path == prefix {
        return String::from("/");
    }

    let Some(rest) = path.strip_prefix(prefix.as_str()) else {
        return path;
    };

    let Some(rest) = rest.strip_prefix('/') else {
        return path;
    };

    if rest.is_empty() {
        String::from("/")
    } else {
        format!("/{}", rest)
    }
}

fn block_on<F>(future: F) -> F::Output
where
    F: Future + Send,
    F::Output: Send,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => match handle.runtime_flavor() {
            tokio::runtime::RuntimeFlavor::MultiThread => {
                tokio::task::block_in_place(|| handle.block_on(future))
            }
            tokio::runtime::RuntimeFlavor::CurrentThread => std::thread::scope(|scope| {
                scope
                    .spawn(|| handle.block_on(future))
                    .join()
                    .expect("application runtime thread panicked")
            }),
            _ => tokio::task::block_in_place(|| handle.block_on(future)),
        },
        Err(_) => tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("application runtime must build")
            .block_on(future),
    }
}

type AppPreflightHook<T> =
    Box<dyn Fn(&T, &AppBootstrapConfig) -> Result<(), AppBuildError> + Send + Sync + 'static>;
pub type AppRouteHandler = nivasa_http::AppRouteHandler;

#[cfg(test)]
mod docs_tests {
    use super::*;
    use nivasa_core::DiError;
    use nivasa_http::{NextMiddleware, NivasaRequest, NivasaResponse};
    use nivasa_routing::RouteMethod;
    use std::io;

    #[test]
    fn default_server_options_are_sane() {
        let options = ServerOptions::default();

        assert_eq!(options.host, "127.0.0.1");
        assert_eq!(options.port, 3000);
        assert!(!options.cors);
        assert_eq!(options.global_prefix, None);
        assert_eq!(options.versioning, None);
        assert_eq!(options.listen_address(), "127.0.0.1:3000");
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

        let blank = VersioningOptions::new(VersioningStrategy::Uri).with_default_version("   ");
        assert_eq!(blank.default_version, None);

        let defaulted = VersioningOptions::default();
        assert_eq!(defaulted.strategy, VersioningStrategy::Uri);
        assert_eq!(defaulted.default_version, None);
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
        let from_builder: VersioningOptions =
            VersioningOptions::builder(VersioningStrategy::Header).into();
        assert_eq!(from_builder.strategy, VersioningStrategy::Header);
        assert_eq!(from_builder.default_version, None);
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

    #[test]
    fn helper_normalization_covers_empty_root_and_uppercase_edges() {
        assert_eq!(normalize_path_prefix("   ".to_string()), "");
        assert_eq!(normalize_path_prefix("/".to_string()), "/");
        assert_eq!(normalize_route_path("   "), "/");
        assert_eq!(normalize_route_path("/users/"), "/users");
        assert_eq!(normalize_version_token(" /V42/ "), "v42");
    }

    #[test]
    fn controller_lookup_path_handles_missing_root_and_non_matching_prefixes() {
        assert_eq!(controller_lookup_path(None, "users"), "/users");
        assert_eq!(controller_lookup_path(Some("/"), "/users"), "/users");
        assert_eq!(controller_lookup_path(Some("/api"), "/api"), "/");
        assert_eq!(controller_lookup_path(Some("/api"), "/api/users"), "/users");
        assert_eq!(
            controller_lookup_path(Some("/api"), "/api/users/nested"),
            "/users/nested"
        );
        assert_eq!(
            controller_lookup_path(Some("/api"), "/other/users"),
            "/other/users"
        );
        assert_eq!(
            controller_lookup_path(Some("/api"), "/apiusers"),
            "/apiusers"
        );
    }

    #[test]
    fn server_options_builder_can_disable_cors_explicitly() {
        let options = ServerOptions::builder()
            .enable_cors()
            .cors(false)
            .global_prefix("   ")
            .build();

        assert!(!options.cors);
        assert_eq!(options.global_prefix, None);

        let direct = ServerOptions::new("localhost", 4000)
            .enable_cors()
            .with_global_prefix("   ")
            .with_versioning(VersioningOptions::default());
        assert!(direct.cors);
        assert_eq!(direct.global_prefix, None);
        assert_eq!(direct.versioning, Some(VersioningOptions::default()));

        let from_builder: ServerOptions = ServerOptions::builder().host("0.0.0.0").into();
        assert_eq!(from_builder.host, "0.0.0.0");
    }

    #[test]
    fn app_build_error_wraps_io_and_di_errors() {
        let listen_error =
            AppBuildError::from(io::Error::new(io::ErrorKind::AddrNotAvailable, "nope"));
        let di_error = AppBuildError::from(DiError::ProviderNotFound("DemoService"));

        assert_eq!(listen_error.to_string(), "listen error: nope");
        assert_eq!(
            di_error.to_string(),
            "Provider not found for type: DemoService"
        );
    }

    #[test]
    fn display_errors_and_startup_report_cover_user_facing_strings() {
        let preflight = AppBuildError::PreflightValidation {
            message: "missing DATABASE_URL".to_string(),
        };
        let duplicate = AppBuildError::DuplicateRoute {
            method: "GET".to_string(),
            path: "/health".to_string(),
        };
        let missing = AppBuildError::MissingRouteHandler {
            handler: "HealthController::check".to_string(),
        };
        let report = AppStartupReport {
            banner: "Nivasa".to_string(),
            root_module: "AppModule",
            routes_registered: 2,
            listen_address: "[::1]:3000".to_string(),
        };

        assert_eq!(
            preflight.to_string(),
            "preflight validation failed: missing DATABASE_URL"
        );
        assert_eq!(
            duplicate.to_string(),
            "duplicate route `GET /health` while building app"
        );
        assert_eq!(
            missing.to_string(),
            "missing route handler `HealthController::check` while building app server"
        );
        assert_eq!(
            report.lines(),
            vec![
                "Nivasa".to_string(),
                "root module loaded: AppModule".to_string(),
                "routes registered: 2".to_string(),
                "listen address: [::1]:3000".to_string(),
            ]
        );
    }

    #[test]
    fn address_and_swagger_path_normalization_cover_edge_cases() {
        let defaulted = AppBootstrapConfig::default()
            .with_openapi_spec_path("   ")
            .with_swagger_ui_path("   ");
        let rooted = AppBootstrapConfig::default().with_swagger_ui_path("/docs/");

        assert_eq!(defaulted.openapi_spec_path(), DEFAULT_OPENAPI_SPEC_PATH);
        assert_eq!(defaulted.swagger_ui_path(), DEFAULT_SWAGGER_UI_PATH);
        assert_eq!(rooted.swagger_ui_path(), "/docs/");
        assert_eq!(format_listen_address("::1", 3000), "[::1]:3000");
        assert_eq!(format_listen_address("[::1]", 3000), "[::1]:3000");
        assert!(startup_banner().contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn resolve_routes_preserves_throttle_metadata_and_rejects_duplicates() {
        struct DemoController;

        let bootstrap =
            AppBootstrapConfig::from(ServerOptions::builder().global_prefix("api").build());
        let registration = ModuleControllerRegistration::new(
            std::any::TypeId::of::<DemoController>(),
            vec![
                nivasa_core::module::ControllerRouteRegistration::new("GET", "/health", "health")
                    .with_throttle(5, 30),
                nivasa_core::module::ControllerRouteRegistration::new("POST", "/", "create")
                    .skip_throttle(),
            ],
            Vec::new(),
        );

        let routes = resolve_routes(&bootstrap, &[registration]).expect("routes should resolve");

        assert_eq!(routes.len(), 2);
        assert_eq!(routes[0].path, "/api/health");
        assert_eq!(
            routes[0]
                .throttle
                .as_ref()
                .map(|throttle| (throttle.limit, throttle.ttl_secs)),
            Some((5, 30))
        );
        assert!(!routes[0].skip_throttle);
        assert_eq!(routes[1].path, "/api");
        assert!(routes[1].skip_throttle);
        assert_eq!(routes[1].throttle, None);

        let duplicate = ModuleControllerRegistration::new(
            std::any::TypeId::of::<DemoController>(),
            vec![
                nivasa_core::module::ControllerRouteRegistration::new("GET", "health", "first"),
                nivasa_core::module::ControllerRouteRegistration::new("GET", "/health", "second"),
            ],
            Vec::new(),
        );

        let error = resolve_routes(&bootstrap, &[duplicate]).expect_err("duplicate should fail");
        match error {
            AppBuildError::DuplicateRoute { method, path } => {
                assert_eq!(method, "GET");
                assert_eq!(path, "/api/health");
            }
            other => panic!("unexpected error: {other}"),
        }
    }
}
