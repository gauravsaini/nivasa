use crate::{
    Body, GuardExecutionOutcome, IntoResponse, NextMiddleware, NivasaMiddleware, NivasaRequest,
    NivasaResponse, RequestPipeline,
};
use bytes::{Bytes, BytesMut};
use futures_util::FutureExt;
use http::{
    header::{
        HeaderMap, HeaderValue, ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_HEADERS,
        ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_REQUEST_HEADERS,
        ACCESS_CONTROL_REQUEST_METHOD, ALLOW, CONTENT_TYPE, ORIGIN,
    },
    Method, Request, Response, StatusCode,
};
use http_body_util::{BodyExt, Full};
use hyper::{body::Incoming, service::service_fn};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder as AutoBuilder;
use nivasa_common::HttpException;
use nivasa_common::RequestContext;
use nivasa_core::module::RouteThrottleRegistration;
use nivasa_filters::HttpExceptionSummary;
use nivasa_filters::{ArgumentsHost, ExceptionFilter, ExceptionFilterMetadata};
use nivasa_guards::{
    ExecutionContext as GuardExecutionContext, Guard, InMemoryThrottlerStorage, ThrottlerGuard,
    ThrottlerStorage,
};
use nivasa_interceptors::{
    CallHandler, ExecutionContext as InterceptorExecutionContext, Interceptor,
};
use nivasa_pipes::{ArgumentMetadata, Pipe};
use nivasa_routing::{
    parse_api_version_accept, parse_api_version_header, RouteDispatchError, RouteDispatchOutcome,
    RouteDispatchRegistry, RouteMethod, RoutePattern, RouteRegistryError,
};
use serde_json::{json, Value};
use std::{
    any::type_name,
    future::Future,
    io,
    net::SocketAddr,
    panic::{catch_unwind, AssertUnwindSafe},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{net::TcpListener, sync::oneshot, task::JoinSet};
#[cfg(feature = "tls")]
use tokio_rustls::TlsAcceptor;
use uuid::Uuid;

type RouteHandler = Arc<dyn Fn(&NivasaRequest) -> NivasaResponse + Send + Sync + 'static>;
type MiddlewareLayer = Arc<dyn NivasaMiddleware + Send + Sync + 'static>;
type GuardLayer = Arc<dyn Guard + Send + Sync + 'static>;
type PipeLayer = Arc<dyn Pipe + Send + Sync + 'static>;
type InterceptorLayer = Arc<dyn Interceptor<Response = NivasaResponse> + Send + Sync + 'static>;
type GlobalFilterLayer =
    Arc<dyn ExceptionFilter<HttpException, NivasaResponse> + Send + Sync + 'static>;

const REQUEST_ID_HEADER: &str = "x-request-id";
const MODULE_NAME_HEADER: &str = "x-module-name";
const USER_ID_HEADER: &str = "x-user-id";
const REQUEST_CONTEXT_REQUEST_ID_KEY: &str = "request_id";
const REQUEST_CONTEXT_USER_ID_KEY: &str = "user_id";
const REQUEST_CONTEXT_MODULE_NAME_KEY: &str = "module_name";

#[derive(Clone)]
pub struct GlobalFilterBinding {
    filter: GlobalFilterLayer,
    exception_type: Option<&'static str>,
    catch_all: bool,
}

impl GlobalFilterBinding {
    pub fn new<F>(filter: F) -> Self
    where
        F: ExceptionFilter<HttpException, NivasaResponse>
            + ExceptionFilterMetadata
            + Send
            + Sync
            + 'static,
    {
        Self {
            exception_type: filter.exception_type(),
            catch_all: filter.is_catch_all(),
            filter: Arc::new(filter),
        }
    }
}

#[derive(Clone)]
struct RouteHandlerBinding {
    handler: RouteHandler,
    module_middlewares: Vec<MiddlewareLayer>,
    module_name: Option<String>,
    handler_filters: Vec<GlobalFilterBinding>,
    controller_filters: Vec<GlobalFilterBinding>,
}

impl RouteHandlerBinding {
    fn new(handler: impl Fn(&NivasaRequest) -> NivasaResponse + Send + Sync + 'static) -> Self {
        Self {
            handler: Arc::new(handler),
            module_middlewares: Vec::new(),
            module_name: None,
            handler_filters: Vec::new(),
            controller_filters: Vec::new(),
        }
    }

    fn with_module_middlewares(mut self, middlewares: Vec<MiddlewareLayer>) -> Self {
        self.module_middlewares = middlewares;
        self
    }

    fn with_module_name(mut self, module_name: impl Into<String>) -> Self {
        self.module_name = Some(module_name.into());
        self
    }

    fn with_handler_filters(mut self, filters: Vec<GlobalFilterBinding>) -> Self {
        self.handler_filters = filters;
        self
    }

    fn with_controller_filters(mut self, filters: Vec<GlobalFilterBinding>) -> Self {
        self.controller_filters = filters;
        self
    }
}

#[derive(Clone)]
struct RouteMiddlewareBinding {
    pattern: RoutePattern,
    excluded_paths: Vec<RoutePattern>,
    middleware: MiddlewareLayer,
}

/// Configuration for transport-level CORS handling.
///
/// ```no_run
/// use http::Method;
/// use nivasa_http::CorsOptions;
///
/// let cors = CorsOptions::permissive()
///     .allow_origins(["https://app.example"])
///     .allow_methods([Method::GET, Method::POST])
///     .allow_headers(["content-type", "authorization"])
///     .allow_credentials(true);
/// # let _ = cors;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CorsOptions {
    allowed_origins: Option<Vec<String>>,
    allowed_methods: Option<Vec<Method>>,
    allowed_headers: Option<Vec<String>>,
    allow_credentials: bool,
}

impl CorsOptions {
    /// Create a permissive CORS configuration that preserves the existing default bridge.
    pub fn permissive() -> Self {
        Self::default()
    }

    /// Restrict allowed origins to a fixed allowlist.
    pub fn allow_origins<I, S>(mut self, origins: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_origins = Some(origins.into_iter().map(Into::into).collect());
        self
    }

    /// Restrict allowed methods for preflight responses.
    pub fn allow_methods<I>(mut self, methods: I) -> Self
    where
        I: IntoIterator<Item = Method>,
    {
        self.allowed_methods = Some(methods.into_iter().collect());
        self
    }

    /// Restrict allowed request headers for preflight responses.
    pub fn allow_headers<I, S>(mut self, headers: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.allowed_headers = Some(headers.into_iter().map(Into::into).collect());
        self
    }

    /// Whether credentialed requests should be allowed.
    pub fn allow_credentials(mut self, allow: bool) -> Self {
        self.allow_credentials = allow;
        self
    }

    fn allow_origin_header_value(&self, request_origin: Option<&str>) -> Option<HeaderValue> {
        if let Some(allowed_origins) = &self.allowed_origins {
            let request_origin = request_origin?;
            if allowed_origins
                .iter()
                .any(|allowed| allowed == request_origin)
            {
                return HeaderValue::from_str(request_origin).ok();
            }

            return None;
        }

        match (self.allow_credentials, request_origin) {
            (true, Some(request_origin)) => HeaderValue::from_str(request_origin).ok(),
            (true, None) => None,
            (false, _) => Some(HeaderValue::from_static("*")),
        }
    }

    fn allow_methods_header_value(&self, headers: &HeaderMap) -> Option<HeaderValue> {
        if let Some(allowed_methods) = &self.allowed_methods {
            if allowed_methods.is_empty() {
                return None;
            }

            let methods = allowed_methods
                .iter()
                .map(Method::as_str)
                .collect::<Vec<_>>()
                .join(", ");

            return HeaderValue::from_str(&methods).ok();
        }

        let requested_method = headers
            .get(ACCESS_CONTROL_REQUEST_METHOD)
            .and_then(|value| value.to_str().ok())?
            .trim();
        if requested_method.is_empty() {
            return None;
        }

        let methods = if requested_method.eq_ignore_ascii_case("OPTIONS") {
            "OPTIONS".to_string()
        } else {
            format!("{requested_method}, OPTIONS")
        };

        HeaderValue::from_str(&methods).ok()
    }

    fn allow_headers_header_value(&self, headers: &HeaderMap) -> Option<HeaderValue> {
        if let Some(allowed_headers) = &self.allowed_headers {
            if allowed_headers.is_empty() {
                return None;
            }

            let headers = allowed_headers.join(", ");
            return HeaderValue::from_str(&headers).ok();
        }

        headers
            .get(ACCESS_CONTROL_REQUEST_HEADERS)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| HeaderValue::from_str(value).ok())
    }

    fn allow_credentials_header_value(&self) -> Option<HeaderValue> {
        self.allow_credentials
            .then_some(HeaderValue::from_static("true"))
    }
}

/// Minimal HTTP transport shell for Nivasa.
///
/// Use the builder to register routes and middleware, then call [`NivasaServer::listen`]
/// to start serving requests.
///
/// ```no_run
/// use nivasa_http::{NivasaResponse, NivasaServer};
/// use nivasa_routing::RouteMethod;
///
/// fn main() -> std::io::Result<()> {
///     let server = NivasaServer::builder()
///         .route(RouteMethod::Get, "/health", |_| NivasaResponse::text("ok"))
///         .expect("route registers")
///         .build();
///
///     let runtime = tokio::runtime::Runtime::new()?;
///     runtime.block_on(server.listen("127.0.0.1", 3000))
/// }
/// ```
pub struct NivasaServer {
    routes: RouteDispatchRegistry<RouteHandlerBinding>,
    middleware: Option<MiddlewareLayer>,
    module_middlewares: Vec<MiddlewareLayer>,
    route_middlewares: Vec<RouteMiddlewareBinding>,
    global_guards: Vec<GuardLayer>,
    global_pipes: Vec<PipeLayer>,
    interceptors: Vec<InterceptorLayer>,
    global_filters: Vec<GlobalFilterBinding>,
    cors: Option<CorsOptions>,
    request_timeout: Option<Duration>,
    request_body_size_limit: Option<usize>,
    shutdown: Option<oneshot::Receiver<()>>,
    #[cfg(feature = "tls")]
    tls_config: Option<Arc<rustls::ServerConfig>>,
}

/// Builder for [`NivasaServer`].
///
/// ```no_run
/// use nivasa_http::{NivasaResponse, NivasaServer};
/// use nivasa_routing::RouteMethod;
///
/// let server = NivasaServer::builder()
///     .route(RouteMethod::Get, "/health", |_| NivasaResponse::text("ok"))
///     .expect("route registers")
///     .build();
/// ```
pub struct NivasaServerBuilder {
    routes: RouteDispatchRegistry<RouteHandlerBinding>,
    middleware: Option<MiddlewareLayer>,
    module_middlewares: Vec<MiddlewareLayer>,
    route_middlewares: Vec<RouteMiddlewareBinding>,
    global_guards: Vec<GuardLayer>,
    global_pipes: Vec<PipeLayer>,
    interceptors: Vec<InterceptorLayer>,
    global_filters: Vec<GlobalFilterBinding>,
    cors: Option<CorsOptions>,
    request_timeout: Option<Duration>,
    request_body_size_limit: Option<usize>,
    throttler_storage: Arc<dyn ThrottlerStorage>,
    shutdown: Option<oneshot::Receiver<()>>,
    #[cfg(feature = "tls")]
    tls_config: Option<Arc<rustls::ServerConfig>>,
}

impl NivasaServer {
    /// Create a new server builder.
    ///
    /// ```no_run
    /// use nivasa_http::{NivasaResponse, NivasaServer};
    /// use nivasa_routing::RouteMethod;
    ///
    /// let server = NivasaServer::builder()
    ///     .route(RouteMethod::Get, "/health", |_| NivasaResponse::text("ok"))
    ///     .expect("route registers")
    ///     .build();
    ///
    /// # let _ = server;
    /// ```
    pub fn builder() -> NivasaServerBuilder {
        NivasaServerBuilder::new()
    }

    /// Start listening for HTTP requests.
    ///
    /// ```no_run
    /// use nivasa_http::{NivasaResponse, NivasaServer};
    /// use nivasa_routing::RouteMethod;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let server = NivasaServer::builder()
    ///         .route(RouteMethod::Get, "/health", |_| NivasaResponse::text("ok"))
    ///         .expect("route registers")
    ///         .build();
    ///
    ///     let runtime = tokio::runtime::Runtime::new()?;
    ///     runtime.block_on(server.listen("127.0.0.1", 3000))
    /// }
    /// ```
    pub async fn listen(mut self, host: impl AsRef<str>, port: u16) -> io::Result<()> {
        let addr = socket_addr(host.as_ref(), port)?;
        let listener = TcpListener::bind(addr).await?;
        let mut shutdown = shutdown_future(self.shutdown.take());
        let routes = self.routes;
        let middleware = self.middleware;
        let module_middlewares = self.module_middlewares;
        let route_middlewares = self.route_middlewares;
        let global_guards = self.global_guards;
        let global_pipes = self.global_pipes;
        let interceptors = self.interceptors;
        let global_filters = self.global_filters;
        let cors = self.cors;
        let request_timeout = self.request_timeout;
        let request_body_size_limit = self.request_body_size_limit;
        #[cfg(feature = "tls")]
        let tls_config = self.tls_config;
        let mut connections = JoinSet::new();

        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    break;
                }
                accept = listener.accept() => {
                    let (stream, _) = accept?;
                    let routes = routes.clone();
                    let middleware = middleware.clone();
                    let module_middlewares = module_middlewares.clone();
                    let route_middlewares = route_middlewares.clone();
                    let global_guards = global_guards.clone();
                    let global_pipes = global_pipes.clone();
                    let interceptors = interceptors.clone();
                    let global_filters_for_connection = global_filters.clone();
                    let cors = cors.clone();
                    #[cfg(feature = "tls")]
                    let tls_config = tls_config.clone();

                    connections.spawn(async move {
                        #[cfg(feature = "tls")]
                        if let Some(tls_config) = tls_config {
                            let acceptor = TlsAcceptor::from(tls_config);
                            if let Ok(stream) = acceptor.accept(stream).await {
                                serve_connection(
                                    stream,
                                    routes,
                                    middleware,
                                    module_middlewares,
                                    route_middlewares,
                                    global_guards,
                                    global_pipes,
                                    interceptors,
                                    global_filters_for_connection,
                                    cors,
                                    request_timeout,
                                    request_body_size_limit,
                                )
                                .await;
                            }
                            return;
                        }

                        serve_connection(
                            stream,
                            routes,
                            middleware,
                            module_middlewares,
                            route_middlewares,
                            global_guards,
                            global_pipes,
                            interceptors,
                            global_filters_for_connection,
                            cors,
                            request_timeout,
                            request_body_size_limit,
                        )
                        .await;
                    });
                }
            }
        }

        while connections.join_next().await.is_some() {}
        Ok(())
    }

    pub(crate) async fn dispatch_for_test(&self, request: NivasaRequest) -> Response<Full<Bytes>> {
        let response = dispatch_nivasa_request(
            request,
            self.routes.clone(),
            self.middleware.clone(),
            self.module_middlewares.clone(),
            self.route_middlewares.clone(),
            self.global_guards.clone(),
            self.global_pipes.clone(),
            self.interceptors.clone(),
            self.global_filters.clone(),
            self.cors.clone(),
        )
        .await;

        build_response(response.status(), response)
    }
}

impl NivasaServerBuilder {
    fn new() -> Self {
        Self {
            routes: RouteDispatchRegistry::new(),
            middleware: None,
            module_middlewares: Vec::new(),
            route_middlewares: Vec::new(),
            global_guards: Vec::new(),
            global_pipes: Vec::new(),
            interceptors: Vec::new(),
            global_filters: Vec::new(),
            cors: None,
            request_timeout: None,
            request_body_size_limit: None,
            throttler_storage: Arc::new(InMemoryThrottlerStorage::new()),
            shutdown: None,
            #[cfg(feature = "tls")]
            tls_config: None,
        }
    }

    /// Register a request handler for a route.
    ///
    /// ```no_run
    /// use nivasa_http::{NivasaResponse, NivasaServer};
    /// use nivasa_routing::RouteMethod;
    ///
    /// let server = NivasaServer::builder()
    ///     .route(RouteMethod::Get, "/health", |_| NivasaResponse::text("ok"))
    ///     .expect("route registers")
    ///     .build();
    ///
    /// # let _ = server;
    /// ```
    pub fn route(
        mut self,
        method: impl Into<RouteMethod>,
        path: impl Into<String>,
        handler: impl Fn(&NivasaRequest) -> NivasaResponse + Send + Sync + 'static,
    ) -> Result<Self, RouteDispatchError> {
        self.routes
            .register_pattern(method, path, RouteHandlerBinding::new(handler))?;
        Ok(self)
    }

    /// Register a request handler with a throttling window.
    pub fn route_with_throttle(
        mut self,
        method: impl Into<RouteMethod>,
        path: impl Into<String>,
        handler: impl Fn(&NivasaRequest) -> NivasaResponse + Send + Sync + 'static,
        throttle: RouteThrottleRegistration,
    ) -> Result<Self, RouteDispatchError> {
        let storage = Arc::clone(&self.throttler_storage);
        let wrapped_handler = move |request: &NivasaRequest| {
            let context = GuardExecutionContext::new(request.clone())
                .with_request_context(request_context_from_request(request));
            let guard = ThrottlerGuard::new(throttle.limit, Duration::from_secs(throttle.ttl_secs))
                .with_storage(Arc::clone(&storage));

            if guard.allows_request(&context) {
                handler(request)
            } else {
                NivasaResponse::new(
                    StatusCode::TOO_MANY_REQUESTS,
                    Body::text("too many requests"),
                )
            }
        };

        self.routes
            .register_pattern(method, path, RouteHandlerBinding::new(wrapped_handler))?;
        Ok(self)
    }

    /// Register a request handler with local handler/controller exception filters.
    pub fn route_with_filters(
        mut self,
        method: impl Into<RouteMethod>,
        path: impl Into<String>,
        handler: impl Fn(&NivasaRequest) -> NivasaResponse + Send + Sync + 'static,
        handler_filters: Vec<GlobalFilterBinding>,
        controller_filters: Vec<GlobalFilterBinding>,
    ) -> Result<Self, RouteDispatchError> {
        self.routes.register_pattern(
            method,
            path,
            RouteHandlerBinding::new(handler)
                .with_handler_filters(handler_filters)
                .with_controller_filters(controller_filters),
        )?;
        Ok(self)
    }

    /// Register a request handler with module-scoped middleware attached to the route.
    pub fn route_with_module_middlewares<M, I>(
        mut self,
        method: impl Into<RouteMethod>,
        path: impl Into<String>,
        module_middlewares: I,
        handler: impl Fn(&NivasaRequest) -> NivasaResponse + Send + Sync + 'static,
    ) -> Result<Self, RouteDispatchError>
    where
        M: NivasaMiddleware + Send + Sync + 'static,
        I: IntoIterator<Item = M>,
    {
        let module_middlewares = module_middlewares
            .into_iter()
            .map(|middleware| Arc::new(middleware) as MiddlewareLayer)
            .collect::<Vec<_>>();
        let module_name = short_type_name(type_name::<M>());

        self.routes.register_pattern(
            method,
            path,
            RouteHandlerBinding::new(handler)
                .with_module_middlewares(module_middlewares)
                .with_module_name(module_name),
        )?;
        Ok(self)
    }

    /// Swap in a custom throttling backend for subsequent throttled routes.
    pub fn use_throttler_storage<S>(mut self, storage: S) -> Self
    where
        S: ThrottlerStorage + 'static,
    {
        self.throttler_storage = Arc::new(storage);
        self
    }

    /// Register a route that is selected by `X-API-Version`.
    pub fn route_header_versioned(
        mut self,
        method: impl Into<RouteMethod>,
        version: impl Into<String>,
        path: impl Into<String>,
        handler: impl Fn(&NivasaRequest) -> NivasaResponse + Send + Sync + 'static,
    ) -> Result<Self, RouteDispatchError> {
        self.routes.register_header_versioned_route(
            method,
            version,
            path,
            RouteHandlerBinding::new(handler),
        )?;
        Ok(self)
    }

    /// Register a route that is selected by an `Accept` media type version.
    pub fn route_media_type_versioned(
        mut self,
        method: impl Into<RouteMethod>,
        version: impl Into<String>,
        path: impl Into<String>,
        handler: impl Fn(&NivasaRequest) -> NivasaResponse + Send + Sync + 'static,
    ) -> Result<Self, RouteDispatchError> {
        self.routes.register_media_type_versioned_route(
            method,
            version,
            path,
            RouteHandlerBinding::new(handler),
        )?;
        Ok(self)
    }

    /// Enable permissive transport-side CORS handling.
    pub fn enable_cors(mut self) -> Self {
        self.cors = Some(CorsOptions::permissive());
        self
    }

    /// Register a single middleware hook around request handling.
    pub fn middleware<M>(mut self, middleware: M) -> Self
    where
        M: NivasaMiddleware + Send + Sync + 'static,
    {
        self.middleware = Some(Arc::new(middleware));
        self
    }

    /// Register a middleware stage between the global hook and route-specific middleware.
    pub fn module_middleware<M>(mut self, middleware: M) -> Self
    where
        M: NivasaMiddleware + Send + Sync + 'static,
    {
        self.module_middlewares.push(Arc::new(middleware));
        self
    }

    /// Register a single guard hook around matched route handling.
    pub fn use_global_guard<G>(mut self, guard: G) -> Self
    where
        G: Guard + Send + Sync + 'static,
    {
        self.global_guards.push(Arc::new(guard));
        self
    }

    /// Register a single pipe hook around matched route handling.
    pub fn use_global_pipe<P>(mut self, pipe: P) -> Self
    where
        P: Pipe + Send + Sync + 'static,
    {
        self.global_pipes.push(Arc::new(pipe));
        self
    }

    /// Start configuring middleware for one or more matched routes.
    pub fn apply<M>(self, middleware: M) -> RouteMiddlewareBuilder
    where
        M: NivasaMiddleware + Send + Sync + 'static,
    {
        RouteMiddlewareBuilder {
            builder: self,
            middleware: Arc::new(middleware),
            excluded_paths: Vec::new(),
        }
    }

    /// Register a single interceptor hook around a matched route handler.
    pub fn interceptor<I>(mut self, interceptor: I) -> Self
    where
        I: Interceptor<Response = NivasaResponse> + Send + Sync + 'static,
    {
        self.interceptors.push(Arc::new(interceptor));
        self
    }

    /// Register a global HTTP exception filter.
    pub fn use_global_filter<F>(mut self, filter: F) -> Self
    where
        F: ExceptionFilter<HttpException, NivasaResponse>
            + ExceptionFilterMetadata
            + Send
            + Sync
            + 'static,
    {
        self.global_filters.push(GlobalFilterBinding::new(filter));
        self
    }

    /// Register a GET endpoint that serves a prebuilt OpenAPI document as JSON.
    pub fn openapi_spec_json(
        self,
        path: impl Into<String>,
        document: Value,
    ) -> Result<Self, RouteDispatchError> {
        let document = Arc::new(document);

        self.route(RouteMethod::Get, path, move |_| {
            NivasaResponse::json(document.as_ref().clone())
        })
    }

    /// Configure transport-side CORS handling with explicit origins, methods, and headers.
    ///
    /// ```no_run
    /// use http::Method;
    /// use nivasa_http::{CorsOptions, NivasaResponse, NivasaServer};
    /// use nivasa_routing::RouteMethod;
    ///
    /// let cors = CorsOptions::permissive()
    ///     .allow_origins(["https://app.example"])
    ///     .allow_methods([Method::GET])
    ///     .allow_headers(["content-type"]);
    ///
    /// let server = NivasaServer::builder()
    ///     .route(RouteMethod::Get, "/health", |_| NivasaResponse::text("ok"))
    ///     .expect("route registers")
    ///     .cors_options(cors)
    ///     .build();
    ///
    /// # let _ = server;
    /// ```
    pub fn cors_options(mut self, cors: CorsOptions) -> Self {
        self.cors = Some(cors);
        self
    }

    /// Toggle permissive transport-side CORS handling explicitly.
    pub fn cors(mut self, cors: bool) -> Self {
        self.cors = cors.then(CorsOptions::permissive);
        self
    }

    /// Configure the maximum amount of time allowed for a request.
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = Some(timeout);
        self
    }

    /// Configure the maximum request body size in bytes.
    pub fn request_body_size_limit(mut self, limit: usize) -> Self {
        self.request_body_size_limit = Some(limit);
        self
    }

    /// Configure rustls transport for accepted connections.
    #[cfg(feature = "tls")]
    pub fn tls_config(mut self, config: rustls::ServerConfig) -> Self {
        self.tls_config = Some(Arc::new(config));
        self
    }

    /// Provide a custom shutdown signal for tests or embeddings.
    pub fn shutdown_signal(mut self, shutdown: oneshot::Receiver<()>) -> Self {
        self.shutdown = Some(shutdown);
        self
    }

    /// Finalize the server.
    pub fn build(self) -> NivasaServer {
        NivasaServer {
            routes: self.routes,
            middleware: self.middleware,
            module_middlewares: self.module_middlewares,
            route_middlewares: self.route_middlewares,
            global_guards: self.global_guards,
            global_pipes: self.global_pipes,
            interceptors: self.interceptors,
            global_filters: self.global_filters,
            cors: self.cors,
            request_timeout: self.request_timeout,
            request_body_size_limit: self.request_body_size_limit,
            shutdown: self.shutdown,
            #[cfg(feature = "tls")]
            tls_config: self.tls_config,
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn serve_connection<S>(
    stream: S,
    routes: RouteDispatchRegistry<RouteHandlerBinding>,
    middleware: Option<MiddlewareLayer>,
    module_middlewares: Vec<MiddlewareLayer>,
    route_middlewares: Vec<RouteMiddlewareBinding>,
    global_guards: Vec<GuardLayer>,
    global_pipes: Vec<PipeLayer>,
    interceptors: Vec<InterceptorLayer>,
    global_filters: Vec<GlobalFilterBinding>,
    cors: Option<CorsOptions>,
    request_timeout: Option<Duration>,
    request_body_size_limit: Option<usize>,
) where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let io = TokioIo::new(stream);
    let service = service_fn(move |request| {
        let routes = routes.clone();
        let middleware = middleware.clone();
        let module_middlewares = module_middlewares.clone();
        let route_middlewares = route_middlewares.clone();
        let global_guards = global_guards.clone();
        let global_pipes = global_pipes.clone();
        let interceptors = interceptors.clone();
        let global_filters = global_filters.clone();
        let cors = cors.clone();
        let request_timeout = request_timeout;
        let request_body_size_limit = request_body_size_limit;
        async move {
            if let Some(timeout) = request_timeout {
                match tokio::time::timeout(
                    timeout,
                    handle_request(
                        request,
                        routes,
                        middleware,
                        module_middlewares,
                        route_middlewares,
                        global_guards,
                        global_pipes,
                        interceptors,
                        global_filters,
                        cors.clone(),
                        request_body_size_limit,
                    ),
                )
                .await
                {
                    Ok(result) => result,
                    Err(_) => Ok(finalize_response(
                        NivasaResponse::new(
                            StatusCode::REQUEST_TIMEOUT,
                            Body::text("request timed out"),
                        ),
                        cors.as_ref(),
                        None,
                        None,
                    )),
                }
            } else {
                handle_request(
                    request,
                    routes,
                    middleware,
                    module_middlewares,
                    route_middlewares,
                    global_guards,
                    global_pipes,
                    interceptors,
                    global_filters,
                    cors.clone(),
                    request_body_size_limit,
                )
                .await
            }
        }
    });

    let builder = AutoBuilder::new(TokioExecutor::new());
    let _ = builder.serve_connection(io, service).await;
}

#[allow(clippy::too_many_arguments)]
async fn handle_request(
    request: hyper::Request<Incoming>,
    routes: RouteDispatchRegistry<RouteHandlerBinding>,
    middleware: Option<MiddlewareLayer>,
    module_middlewares: Vec<MiddlewareLayer>,
    route_middlewares: Vec<RouteMiddlewareBinding>,
    global_guards: Vec<GuardLayer>,
    global_pipes: Vec<PipeLayer>,
    interceptors: Vec<InterceptorLayer>,
    global_filters: Vec<GlobalFilterBinding>,
    cors: Option<CorsOptions>,
    request_body_size_limit: Option<usize>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let (parts, body) = request.into_parts();
    let request_origin = parts
        .headers
        .get(ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let request_id = request_id_from_headers(&parts.headers);

    if cors.is_some() && is_cors_preflight(&parts.headers, &parts.method) {
        return Ok(build_cors_preflight_response(
            &parts.headers,
            cors.as_ref(),
            request_origin.as_deref(),
        ));
    }

    let body = match collect_request_body(body, request_body_size_limit).await {
        Ok(body) => body,
        Err(BodyCollectionError::TooLarge) => {
            return Ok(finalize_response(
                NivasaResponse::new(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    Body::text("request body too large"),
                ),
                cors.as_ref(),
                request_origin.as_deref(),
                Some(request_id.as_str()),
            ));
        }
        Err(BodyCollectionError::Invalid) => {
            return Ok(finalize_response(
                NivasaResponse::new(StatusCode::BAD_REQUEST, Body::text("invalid request body")),
                cors.as_ref(),
                request_origin.as_deref(),
                Some(request_id.as_str()),
            ));
        }
    };

    let body = if body.is_empty() {
        Body::empty()
    } else {
        Body::bytes(body.to_vec())
    };

    let request = NivasaRequest::from_http(Request::from_parts(parts, body));
    let request = seed_request_identity(request, request_id.clone());
    let request_module_name = module_name_for_request(&request, &routes);
    let request = attach_module_name(request, request_module_name.as_deref());
    let request = match middleware {
        Some(middleware) => match execute_middleware(middleware, request).await {
            MiddlewareExecution::Forwarded(request) => request,
            MiddlewareExecution::ShortCircuited { request, response } => {
                let mut pipeline = RequestPipeline::new(request);

                if pipeline.parse_request().is_err() || pipeline.fail_middleware().is_err() {
                    return Ok(finalize_response(
                        NivasaResponse::new(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Body::text("request pipeline middleware transition failed"),
                        ),
                        cors.as_ref(),
                        request_origin.as_deref(),
                        Some(request_id.as_str()),
                    ));
                }

                return Ok(finalize_response(
                    response,
                    cors.as_ref(),
                    request_origin.as_deref(),
                    Some(request_id.as_str()),
                ));
            }
        },
        None => request,
    };
    let mut pipeline = RequestPipeline::new(request);

    if pipeline.parse_request().is_err() {
        return Ok(finalize_response(
            NivasaResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                Body::text("request pipeline parse transition failed"),
            ),
            cors.as_ref(),
            request_origin.as_deref(),
            Some(request_id.as_str()),
        ));
    }

    if pipeline.complete_middleware().is_err() {
        return Ok(finalize_response(
            NivasaResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                Body::text("request pipeline middleware transition failed"),
            ),
            cors.as_ref(),
            request_origin.as_deref(),
            Some(request_id.as_str()),
        ));
    }

    let versioned_routes = versioned_routes_for_request(pipeline.request(), &routes);

    let response = match pipeline.match_route(&versioned_routes) {
        Ok(RouteDispatchOutcome::Matched(entry)) => {
            let binding = entry.value.clone();
            let handler = Arc::clone(&binding.handler);
            let request = pipeline.request().clone();
            let request = attach_module_name(request, binding.module_name.as_deref());
            let module_middlewares = module_middlewares.clone();
            let route_module_middlewares = binding.module_middlewares.clone();
            let route_middlewares =
                matching_route_middlewares(request.path(), route_middlewares.as_slice());
            let request = match execute_middleware_sequence(module_middlewares, request).await {
                MiddlewareExecution::Forwarded(request) => request,
                MiddlewareExecution::ShortCircuited { response, .. } => {
                    return Ok(finalize_response(
                        response,
                        cors.as_ref(),
                        request_origin.as_deref(),
                        Some(request_id.as_str()),
                    ));
                }
            };
            let request = match execute_middleware_sequence(route_module_middlewares, request).await
            {
                MiddlewareExecution::Forwarded(request) => request,
                MiddlewareExecution::ShortCircuited { response, .. } => {
                    return Ok(finalize_response(
                        response,
                        cors.as_ref(),
                        request_origin.as_deref(),
                        Some(request_id.as_str()),
                    ));
                }
            };
            let request = match execute_middleware_sequence(route_middlewares, request).await {
                MiddlewareExecution::Forwarded(request) => request,
                MiddlewareExecution::ShortCircuited { response, .. } => {
                    return Ok(finalize_response(
                        response,
                        cors.as_ref(),
                        request_origin.as_deref(),
                        Some(request_id.as_str()),
                    ));
                }
            };
            *pipeline.request_mut() = request.clone();
            let guard_context = GuardExecutionContext::new(request.clone())
                .with_request_context(request_context_from_request(&request));
            let guard_refs = global_guards
                .iter()
                .map(|guard| guard.as_ref() as &dyn Guard)
                .collect::<Vec<_>>();

            match pipeline
                .evaluate_guard_chain(&guard_refs, &guard_context)
                .await
            {
                Ok(GuardExecutionOutcome::Passed) => {}
                Ok(GuardExecutionOutcome::Denied) => {
                    return Ok(finalize_response(
                        HttpExceptionSummary::from(&HttpException::forbidden(
                            "request denied by guard",
                        ))
                        .into_response(),
                        cors.as_ref(),
                        request_origin.as_deref(),
                        Some(request_id.as_str()),
                    ));
                }
                Ok(GuardExecutionOutcome::Error(error)) => {
                    return Ok(finalize_response(
                        handle_http_exception(
                            error,
                            &binding.handler_filters,
                            &binding.controller_filters,
                            &global_filters,
                            pipeline.request(),
                        )
                        .await,
                        cors.as_ref(),
                        request_origin.as_deref(),
                        Some(request_id.as_str()),
                    ));
                }
                Err(_) => {
                    return Ok(finalize_response(
                        NivasaResponse::new(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Body::text("request pipeline guard transition failed"),
                        ),
                        cors.as_ref(),
                        request_origin.as_deref(),
                        Some(request_id.as_str()),
                    ));
                }
            }

            if pipeline.complete_interceptors_pre().is_err() {
                return Ok(finalize_response(
                    NivasaResponse::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Body::text("request pipeline interceptor transition failed"),
                    ),
                    cors.as_ref(),
                    request_origin.as_deref(),
                    Some(request_id.as_str()),
                ));
            }

            let transformed_body = match apply_global_pipes(pipeline.request(), &global_pipes) {
                Ok(body) => body,
                Err(error) => {
                    if pipeline.fail_pipes().is_err() {
                        return Ok(finalize_response(
                            NivasaResponse::new(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Body::text("request pipeline pipe transition failed"),
                            ),
                            cors.as_ref(),
                            request_origin.as_deref(),
                            Some(request_id.as_str()),
                        ));
                    }

                    return Ok(finalize_response(
                        handle_http_exception(
                            error,
                            &binding.handler_filters,
                            &binding.controller_filters,
                            &global_filters,
                            pipeline.request(),
                        )
                        .await,
                        cors.as_ref(),
                        request_origin.as_deref(),
                        Some(request_id.as_str()),
                    ));
                }
            };
            *pipeline.request_mut().body_mut() = transformed_body;

            if pipeline.complete_pipes().is_err() {
                return Ok(finalize_response(
                    NivasaResponse::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Body::text("request pipeline pipe transition failed"),
                    ),
                    cors.as_ref(),
                    request_origin.as_deref(),
                    Some(request_id.as_str()),
                ));
            }

            let request = pipeline.request().clone();
            match interceptors.is_empty() {
                false => match execute_interceptors(interceptors, request, handler).await {
                    InterceptorExecution::Completed(response) => {
                        if pipeline.complete_handler().is_err()
                            || pipeline.complete_interceptors_post().is_err()
                            || pipeline.complete_response().is_err()
                        {
                            NivasaResponse::new(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Body::text("request pipeline interceptor transition failed"),
                            )
                        } else {
                            with_request_id(
                                map_interceptor_response(response),
                                Some(request_id.as_str()),
                            )
                        }
                    }
                    InterceptorExecution::ShortCircuited(response) => {
                        if pipeline.fail_interceptors_pre().is_err() {
                            NivasaResponse::new(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Body::text("request pipeline interceptor transition failed"),
                            )
                        } else {
                            with_request_id(
                                map_interceptor_response(response),
                                Some(request_id.as_str()),
                            )
                        }
                    }
                    InterceptorExecution::Error {
                        error,
                        handler_called,
                    } => {
                        let transition_failed = if handler_called {
                            pipeline.complete_handler().is_err()
                                || pipeline.fail_interceptors_post().is_err()
                        } else {
                            pipeline.fail_interceptors_pre().is_err()
                        };

                        if transition_failed {
                            NivasaResponse::new(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Body::text("request pipeline interceptor transition failed"),
                            )
                        } else {
                            handle_http_exception(
                                error,
                                &binding.handler_filters,
                                &binding.controller_filters,
                                &global_filters,
                                pipeline.request(),
                            )
                            .await
                        }
                    }
                },
                true => {
                    let request = pipeline.request().clone();
                    match tokio::task::spawn_blocking(move || (handler)(&request)).await {
                        Ok(response) => {
                            if pipeline.complete_handler().is_err()
                                || pipeline.complete_interceptors_post().is_err()
                                || pipeline.complete_response().is_err()
                            {
                                NivasaResponse::new(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Body::text("request pipeline handler transition failed"),
                                )
                            } else {
                                response
                            }
                        }
                        Err(_) => {
                            if pipeline.fail_handler().is_err() {
                                NivasaResponse::new(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Body::text("request pipeline handler transition failed"),
                                )
                            } else {
                                NivasaResponse::new(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Body::text("request handler failed"),
                                )
                            }
                        }
                    }
                }
            }
        }
        Ok(RouteDispatchOutcome::NotFound) => {
            NivasaResponse::new(StatusCode::NOT_FOUND, Body::text("not found"))
        }
        Ok(RouteDispatchOutcome::MethodNotAllowed {
            allowed_methods, ..
        }) => NivasaResponse::new(
            StatusCode::METHOD_NOT_ALLOWED,
            Body::text("method not allowed"),
        )
        .with_header(ALLOW.as_str(), allowed_methods.join(", ")),
        Err(_) => NivasaResponse::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            Body::text("request pipeline route transition failed"),
        ),
    };

    Ok(finalize_response(
        response,
        cors.as_ref(),
        request_origin.as_deref(),
        Some(request_id.as_str()),
    ))
}

#[allow(clippy::too_many_arguments)]
async fn dispatch_nivasa_request(
    request: NivasaRequest,
    routes: RouteDispatchRegistry<RouteHandlerBinding>,
    middleware: Option<MiddlewareLayer>,
    module_middlewares: Vec<MiddlewareLayer>,
    route_middlewares: Vec<RouteMiddlewareBinding>,
    global_guards: Vec<GuardLayer>,
    global_pipes: Vec<PipeLayer>,
    interceptors: Vec<InterceptorLayer>,
    global_filters: Vec<GlobalFilterBinding>,
    cors: Option<CorsOptions>,
) -> NivasaResponse {
    let request_origin = request
        .header(ORIGIN)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let request_id = request_header_value(&request, REQUEST_ID_HEADER)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let request = seed_request_identity(request, request_id.clone());
    let request_module_name = module_name_for_request(&request, &routes);
    let request = attach_module_name(request, request_module_name.as_deref());
    let request = match middleware {
        Some(middleware) => match execute_middleware(middleware, request).await {
            MiddlewareExecution::Forwarded(request) => request,
            MiddlewareExecution::ShortCircuited { request, response } => {
                let mut pipeline = RequestPipeline::new(request);

                if pipeline.parse_request().is_err() || pipeline.fail_middleware().is_err() {
                    return finalize_nivasa_response(
                        NivasaResponse::new(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Body::text("request pipeline middleware transition failed"),
                        ),
                        cors.as_ref(),
                        request_origin.as_deref(),
                        Some(request_id.as_str()),
                    );
                }

                return finalize_nivasa_response(
                    response,
                    cors.as_ref(),
                    request_origin.as_deref(),
                    Some(request_id.as_str()),
                );
            }
        },
        None => request,
    };
    let mut pipeline = RequestPipeline::new(request);

    if pipeline.parse_request().is_err() {
        return finalize_nivasa_response(
            NivasaResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                Body::text("request pipeline parse transition failed"),
            ),
            cors.as_ref(),
            request_origin.as_deref(),
            Some(request_id.as_str()),
        );
    }

    if pipeline.complete_middleware().is_err() {
        return finalize_nivasa_response(
            NivasaResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                Body::text("request pipeline middleware transition failed"),
            ),
            cors.as_ref(),
            request_origin.as_deref(),
            Some(request_id.as_str()),
        );
    }

    let versioned_routes = versioned_routes_for_request(pipeline.request(), &routes);

    let response = match pipeline.match_route(&versioned_routes) {
        Ok(RouteDispatchOutcome::Matched(entry)) => {
            let binding = entry.value.clone();
            let handler = Arc::clone(&binding.handler);
            let request = pipeline.request().clone();
            let request = attach_module_name(request, binding.module_name.as_deref());
            let route_module_middlewares = binding.module_middlewares.clone();
            let route_middlewares =
                matching_route_middlewares(request.path(), route_middlewares.as_slice());
            let request = match execute_middleware_sequence(module_middlewares, request).await {
                MiddlewareExecution::Forwarded(request) => request,
                MiddlewareExecution::ShortCircuited { response, .. } => {
                    return finalize_nivasa_response(
                        response,
                        cors.as_ref(),
                        request_origin.as_deref(),
                        Some(request_id.as_str()),
                    );
                }
            };
            let request = match execute_middleware_sequence(route_module_middlewares, request).await
            {
                MiddlewareExecution::Forwarded(request) => request,
                MiddlewareExecution::ShortCircuited { response, .. } => {
                    return finalize_nivasa_response(
                        response,
                        cors.as_ref(),
                        request_origin.as_deref(),
                        Some(request_id.as_str()),
                    );
                }
            };
            let request = match execute_middleware_sequence(route_middlewares, request).await {
                MiddlewareExecution::Forwarded(request) => request,
                MiddlewareExecution::ShortCircuited { response, .. } => {
                    return finalize_nivasa_response(
                        response,
                        cors.as_ref(),
                        request_origin.as_deref(),
                        Some(request_id.as_str()),
                    );
                }
            };
            *pipeline.request_mut() = request.clone();
            let guard_context = GuardExecutionContext::new(request.clone())
                .with_request_context(request_context_from_request(&request));
            let guard_refs = global_guards
                .iter()
                .map(|guard| guard.as_ref() as &dyn Guard)
                .collect::<Vec<_>>();

            match pipeline
                .evaluate_guard_chain(&guard_refs, &guard_context)
                .await
            {
                Ok(GuardExecutionOutcome::Passed) => {}
                Ok(GuardExecutionOutcome::Denied) => {
                    return finalize_nivasa_response(
                        HttpExceptionSummary::from(&HttpException::forbidden(
                            "request denied by guard",
                        ))
                        .into_response(),
                        cors.as_ref(),
                        request_origin.as_deref(),
                        Some(request_id.as_str()),
                    );
                }
                Ok(GuardExecutionOutcome::Error(error)) => {
                    return finalize_nivasa_response(
                        handle_http_exception(
                            error,
                            &binding.handler_filters,
                            &binding.controller_filters,
                            &global_filters,
                            pipeline.request(),
                        )
                        .await,
                        cors.as_ref(),
                        request_origin.as_deref(),
                        Some(request_id.as_str()),
                    );
                }
                Err(_) => {
                    return finalize_nivasa_response(
                        NivasaResponse::new(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Body::text("request pipeline guard transition failed"),
                        ),
                        cors.as_ref(),
                        request_origin.as_deref(),
                        Some(request_id.as_str()),
                    );
                }
            }

            if pipeline.complete_interceptors_pre().is_err() {
                return finalize_nivasa_response(
                    NivasaResponse::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Body::text("request pipeline interceptor transition failed"),
                    ),
                    cors.as_ref(),
                    request_origin.as_deref(),
                    Some(request_id.as_str()),
                );
            }

            let transformed_body = match apply_global_pipes(pipeline.request(), &global_pipes) {
                Ok(body) => body,
                Err(error) => {
                    if pipeline.fail_pipes().is_err() {
                        return finalize_nivasa_response(
                            NivasaResponse::new(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Body::text("request pipeline pipe transition failed"),
                            ),
                            cors.as_ref(),
                            request_origin.as_deref(),
                            Some(request_id.as_str()),
                        );
                    }

                    return finalize_nivasa_response(
                        handle_http_exception(
                            error,
                            &binding.handler_filters,
                            &binding.controller_filters,
                            &global_filters,
                            pipeline.request(),
                        )
                        .await,
                        cors.as_ref(),
                        request_origin.as_deref(),
                        Some(request_id.as_str()),
                    );
                }
            };
            *pipeline.request_mut().body_mut() = transformed_body;

            if pipeline.complete_pipes().is_err() {
                return finalize_nivasa_response(
                    NivasaResponse::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Body::text("request pipeline pipe transition failed"),
                    ),
                    cors.as_ref(),
                    request_origin.as_deref(),
                    Some(request_id.as_str()),
                );
            }

            let request = pipeline.request().clone();
            match interceptors.is_empty() {
                false => match execute_interceptors(interceptors, request, handler).await {
                    InterceptorExecution::Completed(response) => {
                        if pipeline.complete_handler().is_err()
                            || pipeline.complete_interceptors_post().is_err()
                            || pipeline.complete_response().is_err()
                        {
                            NivasaResponse::new(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Body::text("request pipeline interceptor transition failed"),
                            )
                        } else {
                            map_interceptor_response(response)
                        }
                    }
                    InterceptorExecution::ShortCircuited(response) => {
                        if pipeline.fail_interceptors_pre().is_err() {
                            NivasaResponse::new(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Body::text("request pipeline interceptor transition failed"),
                            )
                        } else {
                            map_interceptor_response(response)
                        }
                    }
                    InterceptorExecution::Error {
                        error,
                        handler_called,
                    } => {
                        let transition_failed = if handler_called {
                            pipeline.complete_handler().is_err()
                                || pipeline.fail_interceptors_post().is_err()
                        } else {
                            pipeline.fail_interceptors_pre().is_err()
                        };

                        if transition_failed {
                            NivasaResponse::new(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Body::text("request pipeline interceptor transition failed"),
                            )
                        } else {
                            handle_http_exception(
                                error,
                                &binding.handler_filters,
                                &binding.controller_filters,
                                &global_filters,
                                pipeline.request(),
                            )
                            .await
                        }
                    }
                },
                true => {
                    let request = pipeline.request().clone();
                    match tokio::task::spawn_blocking(move || (handler)(&request)).await {
                        Ok(response) => {
                            if pipeline.complete_handler().is_err()
                                || pipeline.complete_interceptors_post().is_err()
                                || pipeline.complete_response().is_err()
                            {
                                NivasaResponse::new(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Body::text("request pipeline handler transition failed"),
                                )
                            } else {
                                response
                            }
                        }
                        Err(_) => {
                            if pipeline.fail_handler().is_err() {
                                NivasaResponse::new(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Body::text("request pipeline handler transition failed"),
                                )
                            } else {
                                NivasaResponse::new(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Body::text("request handler failed"),
                                )
                            }
                        }
                    }
                }
            }
        }
        Ok(RouteDispatchOutcome::NotFound) => {
            NivasaResponse::new(StatusCode::NOT_FOUND, Body::text("not found"))
        }
        Ok(RouteDispatchOutcome::MethodNotAllowed {
            allowed_methods, ..
        }) => NivasaResponse::new(
            StatusCode::METHOD_NOT_ALLOWED,
            Body::text("method not allowed"),
        )
        .with_header(ALLOW.as_str(), allowed_methods.join(", ")),
        Err(_) => NivasaResponse::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            Body::text("request pipeline route transition failed"),
        ),
    };

    finalize_nivasa_response(
        response,
        cors.as_ref(),
        request_origin.as_deref(),
        Some(request_id.as_str()),
    )
}

fn request_body_looks_json(request: &NivasaRequest) -> bool {
    request
        .header(CONTENT_TYPE.as_str())
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            value.starts_with("application/json")
                || value.ends_with("+json")
                || value.contains("/json")
        })
        .unwrap_or(false)
}

fn request_body_to_pipe_value(request: &NivasaRequest) -> Result<Value, HttpException> {
    match request.body() {
        Body::Empty => Ok(Value::Null),
        Body::Json(value) => Ok(value.clone()),
        Body::Text(text) | Body::Html(text) => {
            if request_body_looks_json(request) {
                serde_json::from_str(text).map_err(|error| {
                    HttpException::bad_request(format!(
                        "global pipe could not parse request body as JSON: {error}"
                    ))
                })
            } else {
                Ok(Value::String(text.clone()))
            }
        }
        Body::Bytes(bytes) => {
            let text = String::from_utf8(bytes.clone()).map_err(|error| {
                HttpException::bad_request(format!(
                    "global pipe requires a UTF-8 request body: {error}"
                ))
            })?;

            if request_body_looks_json(request) {
                serde_json::from_str(&text).map_err(|error| {
                    HttpException::bad_request(format!(
                        "global pipe could not parse request body as JSON: {error}"
                    ))
                })
            } else {
                Ok(Value::String(text))
            }
        }
    }
}

fn pipe_value_to_body(value: Value) -> Body {
    match value {
        Value::Null => Body::empty(),
        Value::String(text) => Body::text(text),
        other => Body::json(other),
    }
}

fn apply_global_pipes(request: &NivasaRequest, pipes: &[PipeLayer]) -> Result<Body, HttpException> {
    if pipes.is_empty() {
        return Ok(request.body().clone());
    }

    let mut value = request_body_to_pipe_value(request)?;
    let metadata = ArgumentMetadata::new(0).with_data_type("body");

    for pipe in pipes {
        value = pipe.transform(value, metadata.clone())?;
    }

    Ok(pipe_value_to_body(value))
}

enum MiddlewareExecution {
    Forwarded(NivasaRequest),
    ShortCircuited {
        request: NivasaRequest,
        response: NivasaResponse,
    },
}

enum InterceptorExecution {
    Completed(NivasaResponse),
    ShortCircuited(NivasaResponse),
    Error {
        error: HttpException,
        handler_called: bool,
    },
}

async fn handle_http_exception(
    error: HttpException,
    handler_filters: &[GlobalFilterBinding],
    controller_filters: &[GlobalFilterBinding],
    global_filters: &[GlobalFilterBinding],
    request: &NivasaRequest,
) -> NivasaResponse {
    match select_exception_filter(handler_filters)
        .or_else(|| select_exception_filter(controller_filters))
        .or_else(|| select_exception_filter(global_filters))
    {
        Some(filter) => {
            let host =
                ArgumentsHost::new().with_request_context(request_context_from_request(request));
            let fallback_error = error.clone();

            let filter_future =
                match catch_unwind(AssertUnwindSafe(|| filter.filter.catch(error, host))) {
                    Ok(future) => future,
                    Err(_) => return fallback_unhandled_exception_response(&fallback_error),
                };

            match AssertUnwindSafe(filter_future).catch_unwind().await {
                Ok(response) => response,
                Err(_) => fallback_unhandled_exception_response(&fallback_error),
            }
        }
        None => HttpExceptionSummary::from(&error).into_response(),
    }
}

fn fallback_unhandled_exception_response(error: &HttpException) -> NivasaResponse {
    eprintln!("nivasa-http fallback filter handling unhandled exception: {error}");
    HttpExceptionSummary::from(&HttpException::internal_server_error(
        "request handler failed",
    ))
    .into_response()
}

fn select_exception_filter(filters: &[GlobalFilterBinding]) -> Option<&GlobalFilterBinding> {
    let exception_type = std::any::type_name::<HttpException>();

    filters
        .iter()
        .enumerate()
        .fold(None, |best, (index, binding)| {
            let score = match (binding.exception_type, binding.catch_all) {
                (Some(target), _) if target == exception_type => 2,
                (None, true) => 1,
                (None, false) => 0,
                _ => 0,
            };

            if score == 0 {
                return best;
            }

            match best {
                None => Some((index, score, binding)),
                Some((best_index, best_score, best_binding)) => {
                    if score > best_score || (score == best_score && index > best_index) {
                        Some((index, score, binding))
                    } else {
                        Some((best_index, best_score, best_binding))
                    }
                }
            }
        })
        .map(|(_, _, binding)| binding)
}

async fn execute_middleware(
    middleware: MiddlewareLayer,
    request: NivasaRequest,
) -> MiddlewareExecution {
    let forwarded_request = Arc::new(tokio::sync::Mutex::new(None));
    let capture = Arc::clone(&forwarded_request);
    let next = NextMiddleware::new(move |request: NivasaRequest| {
        let capture = Arc::clone(&capture);
        async move {
            *capture.lock().await = Some(request);
            NivasaResponse::new(StatusCode::NO_CONTENT, Body::empty())
        }
    });

    let original_request = request.clone();
    let response = middleware.use_(request, next).await;
    let forwarded_request = forwarded_request.lock().await.take();

    match forwarded_request {
        Some(request) => MiddlewareExecution::Forwarded(request),
        None => MiddlewareExecution::ShortCircuited {
            request: original_request,
            response,
        },
    }
}

fn matching_route_middlewares(
    request_path: &str,
    route_middlewares: &[RouteMiddlewareBinding],
) -> Vec<MiddlewareLayer> {
    route_middlewares
        .iter()
        .filter(|binding| {
            binding.pattern.matches(request_path)
                && !binding
                    .excluded_paths
                    .iter()
                    .any(|pattern| pattern.matches(request_path))
        })
        .map(|binding| Arc::clone(&binding.middleware))
        .collect()
}

async fn execute_middleware_sequence(
    middlewares: Vec<MiddlewareLayer>,
    request: NivasaRequest,
) -> MiddlewareExecution {
    let mut request = request;

    for middleware in middlewares {
        let current_request = request.clone();
        match execute_middleware(middleware, current_request).await {
            MiddlewareExecution::Forwarded(next_request) => {
                request = next_request;
            }
            MiddlewareExecution::ShortCircuited { response, .. } => {
                return MiddlewareExecution::ShortCircuited { request, response };
            }
        }
    }

    MiddlewareExecution::Forwarded(request)
}

async fn execute_interceptors(
    interceptors: Vec<InterceptorLayer>,
    request: NivasaRequest,
    handler: RouteHandler,
) -> InterceptorExecution {
    let handler_called = Arc::new(AtomicBool::new(false));
    let context = InterceptorExecutionContext::new()
        .with_request(request.method().to_string(), request.path().to_string());
    let next = interceptor_chain_handler(
        Arc::new(interceptors),
        0,
        context.clone(),
        Arc::clone(&handler_called),
        request,
        handler,
    );

    match next.handle().await {
        Ok(response) => {
            if handler_called.load(Ordering::SeqCst) {
                InterceptorExecution::Completed(response)
            } else {
                InterceptorExecution::ShortCircuited(response)
            }
        }
        Err(error) => InterceptorExecution::Error {
            error,
            handler_called: handler_called.load(Ordering::SeqCst),
        },
    }
}

fn map_interceptor_response(response: NivasaResponse) -> NivasaResponse {
    let status = response.status();
    let headers = response.headers().clone();
    let mapped_body = json!({
        "data": match response.body() {
            Body::Empty => serde_json::Value::Null,
            Body::Text(value) | Body::Html(value) => serde_json::Value::String(value.clone()),
            Body::Json(value) => value.clone(),
            Body::Bytes(bytes) => serde_json::Value::Array(
                bytes.iter().copied().map(serde_json::Value::from).collect(),
            ),
        }
    });

    let mut mapped_response = NivasaResponse::new(status, Body::json(mapped_body));
    for (name, value) in headers.iter() {
        if name.as_str() != CONTENT_TYPE.as_str() {
            if let Ok(value) = value.to_str() {
                mapped_response = mapped_response.with_header(name.as_str(), value);
            }
        }
    }

    mapped_response
}

fn interceptor_chain_handler(
    interceptors: Arc<Vec<InterceptorLayer>>,
    index: usize,
    context: InterceptorExecutionContext,
    handler_called: Arc<AtomicBool>,
    request: NivasaRequest,
    handler: RouteHandler,
) -> CallHandler<NivasaResponse> {
    match interceptors.get(index).cloned() {
        Some(interceptor) => CallHandler::new(move || {
            let interceptors = Arc::clone(&interceptors);
            let context = context.clone();
            let handler_called = Arc::clone(&handler_called);
            let request = request.clone();
            let handler = Arc::clone(&handler);
            async move {
                let next = interceptor_chain_handler(
                    interceptors,
                    index + 1,
                    context.clone(),
                    handler_called,
                    request,
                    handler,
                );
                interceptor.intercept(&context, next).await
            }
        }),
        None => CallHandler::new(move || {
            let handler_called = Arc::clone(&handler_called);
            let request = request.clone();
            let handler = Arc::clone(&handler);
            async move {
                handler_called.store(true, Ordering::SeqCst);
                tokio::task::spawn_blocking(move || (handler)(&request))
                    .await
                    .map_err(|_| HttpException::internal_server_error("request handler failed"))
            }
        }),
    }
}

async fn collect_request_body(
    mut body: Incoming,
    limit: Option<usize>,
) -> Result<Bytes, BodyCollectionError> {
    let mut bytes = BytesMut::new();

    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|_| BodyCollectionError::Invalid)?;
        if let Ok(data) = frame.into_data() {
            if let Some(limit) = limit {
                if bytes.len().saturating_add(data.len()) > limit {
                    return Err(BodyCollectionError::TooLarge);
                }
            }
            bytes.extend_from_slice(&data);
        }
    }

    Ok(bytes.freeze())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyCollectionError {
    Invalid,
    TooLarge,
}

fn versioned_routes_for_request(
    request: &NivasaRequest,
    routes: &RouteDispatchRegistry<RouteHandlerBinding>,
) -> RouteDispatchRegistry<RouteHandlerBinding> {
    let version = request
        .headers()
        .get("X-API-Version")
        .and_then(|value| value.to_str().ok())
        .and_then(parse_api_version_header)
        .or_else(|| {
            request
                .headers()
                .get(http::header::ACCEPT)
                .and_then(|value| value.to_str().ok())
                .and_then(parse_api_version_accept)
        });

    let mut selected = RouteDispatchRegistry::new();

    let mut saw_versioned_match = false;
    if let Some(version) = version.as_deref() {
        for entry in routes.iter() {
            if entry.version.as_deref() == Some(version) && entry.pattern.matches(request.path()) {
                saw_versioned_match = true;
            }
        }
    }

    for entry in routes.iter() {
        let matches_path = entry.pattern.matches(request.path());
        if !matches_path {
            continue;
        }

        let should_include = match version.as_deref() {
            Some(version) if saw_versioned_match => entry.version.as_deref() == Some(version),
            Some(_) => entry.version.is_none(),
            None => entry.version.is_none(),
        };

        if should_include {
            let _ = selected.register(
                entry.method.clone(),
                entry.pattern.clone(),
                entry.value.clone(),
            );
        }
    }

    selected
}

fn build_response(status: StatusCode, response: NivasaResponse) -> Response<Full<Bytes>> {
    let (parts, body) = response.into_inner().into_parts();
    let mut hyper_response = Response::from_parts(parts, Full::new(body.into_shared_bytes()));
    *hyper_response.status_mut() = status;
    hyper_response
}

fn with_request_id(response: NivasaResponse, request_id: Option<&str>) -> NivasaResponse {
    match request_id {
        Some(request_id) => response.with_header(REQUEST_ID_HEADER, request_id),
        None => response,
    }
}

fn attach_module_name(mut request: NivasaRequest, module_name: Option<&str>) -> NivasaRequest {
    if let Some(module_name) = module_name {
        request.set_header(MODULE_NAME_HEADER, module_name);
    }

    request
}

fn request_header_value(request: &NivasaRequest, name: &str) -> Option<String> {
    request
        .header(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn request_id_from_headers(headers: &http::HeaderMap) -> String {
    headers
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn seed_request_identity(
    mut request: NivasaRequest,
    request_id: impl Into<String>,
) -> NivasaRequest {
    let request_id = request_id.into();
    request.set_header(REQUEST_ID_HEADER, &request_id);
    request
}

fn request_context_from_request(request: &NivasaRequest) -> RequestContext {
    let mut request_context = RequestContext::new();
    request_context.insert_request_data(request.clone());
    request_context.set_custom_data(
        "request_method",
        json!(request.method().as_str().to_string()),
    );
    request_context.set_custom_data("request_path", json!(request.path().to_string()));
    if let Some(authorization) = request_header_value(request, "authorization") {
        request_context.set_custom_data("authorization", json!(authorization));
    }

    if let Some(request_id) = request_header_value(request, REQUEST_ID_HEADER) {
        request_context.set_custom_data(REQUEST_CONTEXT_REQUEST_ID_KEY, json!(request_id));
    }
    if let Some(user_id) = request_header_value(request, USER_ID_HEADER) {
        request_context.set_custom_data(REQUEST_CONTEXT_USER_ID_KEY, json!(user_id));
    }
    if let Some(module_name) = request_header_value(request, MODULE_NAME_HEADER) {
        request_context.set_custom_data(REQUEST_CONTEXT_MODULE_NAME_KEY, json!(module_name));
    }

    request_context
}

fn module_name_for_request(
    request: &NivasaRequest,
    routes: &RouteDispatchRegistry<RouteHandlerBinding>,
) -> Option<String> {
    let version = request
        .headers()
        .get("X-API-Version")
        .and_then(|value| value.to_str().ok())
        .and_then(parse_api_version_header)
        .or_else(|| {
            request
                .headers()
                .get(http::header::ACCEPT)
                .and_then(|value| value.to_str().ok())
                .and_then(parse_api_version_accept)
        });

    routes
        .select_versioned(request.path(), version.as_deref())
        .resolve_entry(request.method().as_str())
        .and_then(|entry| entry.value.module_name.clone())
}

fn short_type_name(type_name: &str) -> String {
    type_name
        .rsplit("::")
        .next()
        .unwrap_or(type_name)
        .to_string()
}

fn finalize_response(
    response: NivasaResponse,
    cors: Option<&CorsOptions>,
    request_origin: Option<&str>,
    request_id: Option<&str>,
) -> Response<Full<Bytes>> {
    let response = finalize_nivasa_response(response, cors, request_origin, request_id);
    build_response(response.status(), response)
}

fn finalize_nivasa_response(
    response: NivasaResponse,
    cors: Option<&CorsOptions>,
    request_origin: Option<&str>,
    request_id: Option<&str>,
) -> NivasaResponse {
    let response = with_request_id(response, request_id);
    apply_cors_headers_to_response(response, cors, request_origin)
}

pub struct RouteMiddlewareBuilder {
    builder: NivasaServerBuilder,
    middleware: MiddlewareLayer,
    excluded_paths: Vec<RoutePattern>,
}

impl RouteMiddlewareBuilder {
    /// Exclude an exact request path from the middleware binding.
    pub fn exclude(mut self, path: impl Into<String>) -> Result<Self, RouteRegistryError> {
        let pattern = RoutePattern::static_path(path)?;
        self.excluded_paths.push(pattern);
        Ok(self)
    }

    /// Apply the middleware to one or more matched routes.
    pub fn for_routes(
        mut self,
        path: impl Into<String>,
    ) -> Result<NivasaServerBuilder, RouteRegistryError> {
        let pattern = RoutePattern::parse(path)?;
        self.builder.route_middlewares.push(RouteMiddlewareBinding {
            pattern,
            excluded_paths: self.excluded_paths,
            middleware: self.middleware,
        });
        Ok(self.builder)
    }
}

fn is_cors_preflight(headers: &HeaderMap, method: &Method) -> bool {
    *method == Method::OPTIONS
        && headers.contains_key(ORIGIN)
        && headers.contains_key(ACCESS_CONTROL_REQUEST_METHOD)
}

fn build_cors_preflight_response(
    headers: &HeaderMap,
    cors: Option<&CorsOptions>,
    request_origin: Option<&str>,
) -> Response<Full<Bytes>> {
    let response = build_cors_preflight_nivasa_response(headers, cors, request_origin);
    build_response(response.status(), response)
}

fn build_cors_preflight_nivasa_response(
    headers: &HeaderMap,
    cors: Option<&CorsOptions>,
    request_origin: Option<&str>,
) -> NivasaResponse {
    let mut response = NivasaResponse::new(StatusCode::NO_CONTENT, Body::empty());
    response = apply_cors_headers_to_response(response, cors, request_origin);

    if let Some(value) = allow_methods_header_value(headers, cors) {
        if let Ok(value) = value.to_str() {
            response = response.with_header(ACCESS_CONTROL_ALLOW_METHODS.as_str(), value);
        }
    }

    if let Some(value) = allow_headers_header_value(headers, cors) {
        if let Ok(value) = value.to_str() {
            response = response.with_header(ACCESS_CONTROL_ALLOW_HEADERS.as_str(), value);
        }
    }

    if let Some(cors) = cors {
        if let Some(value) = cors.allow_credentials_header_value() {
            if let Ok(value) = value.to_str() {
                response = response.with_header(ACCESS_CONTROL_ALLOW_CREDENTIALS.as_str(), value);
            }
        }
    }

    response
}

fn apply_cors_headers_to_response(
    mut response: NivasaResponse,
    cors: Option<&CorsOptions>,
    request_origin: Option<&str>,
) -> NivasaResponse {
    let Some(cors) = cors else {
        return response;
    };

    if let Some(value) = cors.allow_origin_header_value(request_origin) {
        if let Ok(value) = value.to_str() {
            response = response.with_header(ACCESS_CONTROL_ALLOW_ORIGIN.as_str(), value);
        }
    }

    if let Some(value) = cors.allow_credentials_header_value() {
        if let Ok(value) = value.to_str() {
            response = response.with_header(ACCESS_CONTROL_ALLOW_CREDENTIALS.as_str(), value);
        }
    }

    response
}

fn allow_methods_header_value(
    headers: &HeaderMap,
    cors: Option<&CorsOptions>,
) -> Option<HeaderValue> {
    match cors {
        Some(cors) => cors.allow_methods_header_value(headers),
        None => None,
    }
}

fn allow_headers_header_value(
    headers: &HeaderMap,
    cors: Option<&CorsOptions>,
) -> Option<HeaderValue> {
    match cors {
        Some(cors) => cors.allow_headers_header_value(headers),
        None => None,
    }
}

fn socket_addr(host: &str, port: u16) -> io::Result<SocketAddr> {
    format!("{}:{}", host.trim(), port).parse().map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid listen address: {err}"),
        )
    })
}

fn shutdown_future(
    shutdown: Option<oneshot::Receiver<()>>,
) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
    match shutdown {
        Some(shutdown) => Box::pin(async move {
            let _ = shutdown.await;
        }),
        None => Box::pin(async move {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{signal, SignalKind};

                if let Ok(mut sigterm) = signal(SignalKind::terminate()) {
                    tokio::select! {
                        _ = tokio::signal::ctrl_c() => {}
                        _ = sigterm.recv() => {}
                    }
                } else {
                    let _ = tokio::signal::ctrl_c().await;
                }
            }

            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::header::AUTHORIZATION;
    use nivasa_filters::{
        ArgumentsHost, ExceptionFilter, ExceptionFilterFuture, ExceptionFilterMetadata,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    #[derive(Clone)]
    struct TestFilter {
        label: &'static str,
        specific: bool,
        catch_all: bool,
        panic_sync: bool,
        panic_async: bool,
    }

    impl ExceptionFilter<HttpException, NivasaResponse> for TestFilter {
        fn catch<'a>(
            &'a self,
            exception: HttpException,
            _host: ArgumentsHost,
        ) -> ExceptionFilterFuture<'a, NivasaResponse> {
            if self.panic_sync {
                panic!("sync filter panic");
            }

            let label = self.label;
            let panic_async = self.panic_async;
            Box::pin(async move {
                if panic_async {
                    panic!("async filter panic");
                }

                NivasaResponse::new(
                    StatusCode::from_u16(exception.status_code)
                        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    Body::text(label),
                )
                .with_header("x-filter-label", label)
            })
        }
    }

    impl ExceptionFilterMetadata for TestFilter {
        fn exception_type(&self) -> Option<&'static str> {
            self.specific
                .then_some(std::any::type_name::<HttpException>())
        }

        fn is_catch_all(&self) -> bool {
            self.catch_all
        }
    }

    #[test]
    fn cors_options_cover_allowlist_and_reflection_edges() {
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCESS_CONTROL_REQUEST_METHOD,
            HeaderValue::from_static("PATCH"),
        );
        headers.insert(
            ACCESS_CONTROL_REQUEST_HEADERS,
            HeaderValue::from_static("authorization, content-type"),
        );

        let allowlist = CorsOptions::permissive()
            .allow_origins(["https://app.example"])
            .allow_methods([Method::GET, Method::POST])
            .allow_headers(["authorization", "content-type"])
            .allow_credentials(true);

        assert_eq!(
            allowlist
                .allow_origin_header_value(Some("https://app.example"))
                .and_then(|value| value.to_str().ok().map(str::to_owned)),
            Some("https://app.example".to_string())
        );
        assert_eq!(
            allowlist
                .allow_origin_header_value(Some("https://evil.example"))
                .and_then(|value| value.to_str().ok().map(str::to_owned)),
            None::<String>
        );
        assert_eq!(
            allowlist
                .allow_methods_header_value(&headers)
                .and_then(|value| value.to_str().ok().map(str::to_owned)),
            Some("GET, POST".to_string())
        );
        assert_eq!(
            allowlist
                .allow_headers_header_value(&headers)
                .and_then(|value| value.to_str().ok().map(str::to_owned)),
            Some("authorization, content-type".to_string())
        );
        assert_eq!(
            allowlist
                .allow_credentials_header_value()
                .and_then(|value| value.to_str().ok().map(str::to_owned)),
            Some("true".to_string())
        );

        let reflective = CorsOptions::permissive().allow_credentials(true);
        assert_eq!(
            reflective
                .allow_origin_header_value(Some("https://client.example"))
                .and_then(|value| value.to_str().ok().map(str::to_owned)),
            Some("https://client.example".to_string())
        );
        assert_eq!(
            reflective
                .allow_origin_header_value(None)
                .and_then(|value| value.to_str().ok().map(str::to_owned)),
            None::<String>
        );

        let wildcard = CorsOptions::permissive();
        assert_eq!(
            wildcard
                .allow_origin_header_value(None)
                .and_then(|value| value.to_str().ok().map(str::to_owned)),
            Some("*".to_string())
        );
        assert_eq!(
            wildcard
                .allow_methods_header_value(&headers)
                .and_then(|value| value.to_str().ok().map(str::to_owned)),
            Some("PATCH, OPTIONS".to_string())
        );
        assert_eq!(
            wildcard
                .allow_headers_header_value(&headers)
                .and_then(|value| value.to_str().ok().map(str::to_owned)),
            Some("authorization, content-type".to_string())
        );
    }

    #[test]
    fn request_body_helpers_cover_json_and_error_paths() {
        let mut json_text =
            NivasaRequest::new(Method::POST, "/users", Body::text("{\"name\":\"Ada\"}"));
        json_text.set_header(CONTENT_TYPE.as_str(), "application/json; charset=utf-8");
        assert!(request_body_looks_json(&json_text));
        assert_eq!(
            request_body_to_pipe_value(&json_text).unwrap(),
            json!({ "name": "Ada" })
        );

        let mut json_bytes = NivasaRequest::new(
            Method::POST,
            "/users",
            Body::bytes(br#"{"enabled":true}"#.to_vec()),
        );
        json_bytes.set_header(CONTENT_TYPE.as_str(), "application/vnd.api+json");
        assert!(request_body_looks_json(&json_bytes));
        assert_eq!(
            request_body_to_pipe_value(&json_bytes).unwrap(),
            json!({ "enabled": true })
        );

        let plain_text = NivasaRequest::new(Method::POST, "/users", Body::text("hello"));
        assert!(!request_body_looks_json(&plain_text));
        assert_eq!(
            request_body_to_pipe_value(&plain_text).unwrap(),
            Value::String("hello".to_string())
        );

        let mut invalid_json = NivasaRequest::new(Method::POST, "/users", Body::text("{nope"));
        invalid_json.set_header(CONTENT_TYPE.as_str(), "application/json");
        let invalid_json_error = request_body_to_pipe_value(&invalid_json).unwrap_err();
        assert_eq!(
            invalid_json_error.status_code,
            StatusCode::BAD_REQUEST.as_u16()
        );
        assert!(invalid_json_error
            .message
            .contains("global pipe could not parse request body as JSON"));

        let invalid_utf8 =
            NivasaRequest::new(Method::POST, "/users", Body::bytes(vec![0x80, 0x81, 0x82]));
        let invalid_utf8_error = request_body_to_pipe_value(&invalid_utf8).unwrap_err();
        assert_eq!(
            invalid_utf8_error.status_code,
            StatusCode::BAD_REQUEST.as_u16()
        );
        assert!(invalid_utf8_error
            .message
            .contains("global pipe requires a UTF-8 request body"));

        assert_eq!(pipe_value_to_body(Value::Null), Body::Empty);
        assert_eq!(
            pipe_value_to_body(Value::String("trimmed".to_string())),
            Body::Text("trimmed".to_string())
        );
        assert_eq!(
            pipe_value_to_body(json!({ "ok": true })),
            Body::Json(json!({ "ok": true }))
        );
    }

    #[test]
    fn request_identity_and_module_helpers_cover_trimmed_and_versioned_paths() {
        let mut request = NivasaRequest::new(Method::GET, "/users", Body::empty());
        request.set_header(AUTHORIZATION.as_str(), " Bearer header.payload.signature ");
        request.set_header(REQUEST_ID_HEADER, " req-123 ");
        request.set_header(USER_ID_HEADER, " user-7 ");
        request.set_header(MODULE_NAME_HEADER, " UsersModule ");

        assert_eq!(
            request_header_value(&request, AUTHORIZATION.as_str()),
            Some("Bearer header.payload.signature".to_string())
        );
        assert_eq!(
            request_header_value(&request, REQUEST_ID_HEADER),
            Some("req-123".to_string())
        );

        let seeded = seed_request_identity(
            NivasaRequest::new(Method::GET, "/seeded", Body::empty()),
            "seed-42",
        );
        assert_eq!(
            seeded
                .header(REQUEST_ID_HEADER)
                .and_then(|value| value.to_str().ok().map(str::to_owned)),
            Some("seed-42".to_string())
        );

        let mut blank_headers = HeaderMap::new();
        blank_headers.insert(REQUEST_ID_HEADER, HeaderValue::from_static("   "));
        let generated_request_id = request_id_from_headers(&blank_headers);
        assert!(!generated_request_id.trim().is_empty());
        assert_ne!(generated_request_id, "   ");

        let mut routes = RouteDispatchRegistry::new();
        routes
            .register(
                RouteMethod::Get,
                RoutePattern::parse("/users".to_string()).unwrap(),
                RouteHandlerBinding::new(|_| NivasaResponse::new(StatusCode::OK, Body::empty()))
                    .with_module_name("FallbackUsersModule"),
            )
            .unwrap();
        routes
            .register_versioned(
                RouteMethod::Get,
                RoutePattern::parse("/users".to_string()).unwrap(),
                Some("v2".to_string()),
                RouteHandlerBinding::new(|_| NivasaResponse::new(StatusCode::OK, Body::empty()))
                    .with_module_name("UsersV2Module"),
            )
            .unwrap();
        routes
            .register_versioned(
                RouteMethod::Get,
                RoutePattern::parse("/users".to_string()).unwrap(),
                Some("v3".to_string()),
                RouteHandlerBinding::new(|_| NivasaResponse::new(StatusCode::OK, Body::empty()))
                    .with_module_name("UsersV3Module"),
            )
            .unwrap();

        let mut header_versioned_request = NivasaRequest::new(Method::GET, "/users", Body::empty());
        header_versioned_request.set_header("X-API-Version", " 2 ");
        assert_eq!(
            module_name_for_request(&header_versioned_request, &routes),
            Some("UsersV2Module".to_string())
        );

        let mut accept_versioned_request = NivasaRequest::new(Method::GET, "/users", Body::empty());
        accept_versioned_request
            .set_header(http::header::ACCEPT.as_str(), "application/vnd.app.v3+json");
        assert_eq!(
            module_name_for_request(&accept_versioned_request, &routes),
            Some("UsersV3Module".to_string())
        );

        let unversioned_request = NivasaRequest::new(Method::GET, "/users", Body::empty());
        assert_eq!(
            module_name_for_request(&unversioned_request, &routes),
            Some("FallbackUsersModule".to_string())
        );

        assert_eq!(short_type_name("demo::users::UsersModule"), "UsersModule");
        assert_eq!(short_type_name("UsersModule"), "UsersModule");
    }

    #[test]
    fn cors_finalization_and_address_helpers_cover_none_and_trimmed_edges() {
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCESS_CONTROL_REQUEST_METHOD,
            HeaderValue::from_static("PUT"),
        );
        headers.insert(
            ACCESS_CONTROL_REQUEST_HEADERS,
            HeaderValue::from_static("x-token"),
        );

        assert!(allow_methods_header_value(&headers, None).is_none());
        assert!(allow_headers_header_value(&headers, None).is_none());

        let preflight = build_cors_preflight_nivasa_response(&headers, None, None);
        assert_eq!(preflight.status(), StatusCode::NO_CONTENT);
        assert!(preflight
            .headers()
            .get(ACCESS_CONTROL_ALLOW_METHODS)
            .is_none());
        assert!(preflight
            .headers()
            .get(ACCESS_CONTROL_ALLOW_HEADERS)
            .is_none());

        let cors = CorsOptions::permissive().allow_credentials(true);
        let finalized = finalize_nivasa_response(
            NivasaResponse::text("ok"),
            Some(&cors),
            Some("https://client.example"),
            Some("req-123"),
        );
        assert_eq!(
            finalized.headers().get(REQUEST_ID_HEADER).unwrap(),
            "req-123"
        );
        assert_eq!(
            finalized
                .headers()
                .get(ACCESS_CONTROL_ALLOW_ORIGIN)
                .unwrap(),
            "https://client.example"
        );
        assert_eq!(
            finalized
                .headers()
                .get(ACCESS_CONTROL_ALLOW_CREDENTIALS)
                .unwrap(),
            "true"
        );

        let response = finalize_response(
            NivasaResponse::text("ok"),
            Some(&cors),
            Some("https://client.example"),
            Some("req-456"),
        );
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(REQUEST_ID_HEADER).unwrap(),
            "req-456"
        );
        assert_eq!(
            socket_addr(" 127.0.0.1 ", 3000)
                .expect("trimmed address should parse")
                .to_string(),
            "127.0.0.1:3000"
        );
    }

    #[test]
    fn versioned_route_selection_covers_fallback_and_path_mismatch_edges() {
        let mut routes = RouteDispatchRegistry::new();
        let fallback = RouteHandlerBinding::new(|_| NivasaResponse::text("fallback"));
        let users_v2 = RouteHandlerBinding::new(|_| NivasaResponse::text("v2"));
        let reports_v2 = RouteHandlerBinding::new(|_| NivasaResponse::text("reports"));

        routes
            .register(
                RouteMethod::Get,
                RoutePattern::parse("/users".to_string()).unwrap(),
                fallback,
            )
            .unwrap();
        routes
            .register_versioned(
                RouteMethod::Get,
                RoutePattern::parse("/users".to_string()).unwrap(),
                Some("v2".to_string()),
                users_v2,
            )
            .unwrap();
        routes
            .register_versioned(
                RouteMethod::Get,
                RoutePattern::parse("/reports".to_string()).unwrap(),
                Some("v2".to_string()),
                reports_v2,
            )
            .unwrap();

        let mut exact = NivasaRequest::new(Method::GET, "/users", Body::empty());
        exact.set_header("X-API-Version", "2");
        let selected = versioned_routes_for_request(&exact, &routes);
        assert!(selected.resolve_entry("GET", "/users").is_some());
        assert_eq!(selected.len(), 1);

        let mut fallback_request = NivasaRequest::new(Method::GET, "/users", Body::empty());
        fallback_request.set_header("X-API-Version", "9");
        let selected = versioned_routes_for_request(&fallback_request, &routes);
        assert!(selected.resolve_entry("GET", "/users").is_some());
        assert_eq!(selected.len(), 1);

        let no_path_match = NivasaRequest::new(Method::GET, "/missing", Body::empty());
        let selected = versioned_routes_for_request(&no_path_match, &routes);
        assert!(selected.is_empty());
    }

    #[test]
    fn request_context_from_request_seeds_request_metadata_and_authorization() {
        let mut request = NivasaRequest::new(Method::POST, "/users/42", Body::text("payload"));
        request.set_header(AUTHORIZATION.as_str(), "Bearer header.payload.signature");
        request.set_header(REQUEST_ID_HEADER, "req-123");
        request.set_header(USER_ID_HEADER, "user-7");
        request.set_header(MODULE_NAME_HEADER, "UsersModule");

        let request_context = request_context_from_request(&request);

        assert_eq!(
            request_context
                .custom_data("request_method")
                .and_then(|value| value.as_str()),
            Some("POST")
        );
        assert_eq!(
            request_context
                .custom_data("request_path")
                .and_then(|value| value.as_str()),
            Some("/users/42")
        );
        assert_eq!(
            request_context
                .custom_data("authorization")
                .and_then(|value| value.as_str()),
            Some("Bearer header.payload.signature")
        );
        assert_eq!(
            request_context
                .custom_data(REQUEST_CONTEXT_REQUEST_ID_KEY)
                .and_then(|value| value.as_str()),
            Some("req-123")
        );
        assert_eq!(
            request_context
                .custom_data(REQUEST_CONTEXT_USER_ID_KEY)
                .and_then(|value| value.as_str()),
            Some("user-7")
        );
        assert_eq!(
            request_context
                .custom_data(REQUEST_CONTEXT_MODULE_NAME_KEY)
                .and_then(|value| value.as_str()),
            Some("UsersModule")
        );
    }

    #[tokio::test]
    async fn exception_filter_selection_prefers_specific_binding_and_latest_catch_all() {
        let request = NivasaRequest::new(Method::GET, "/filters", Body::empty());
        let exact = handle_http_exception(
            HttpException::bad_request("bad"),
            &[
                GlobalFilterBinding::new(TestFilter {
                    label: "early-catch-all",
                    specific: false,
                    catch_all: true,
                    panic_sync: false,
                    panic_async: false,
                }),
                GlobalFilterBinding::new(TestFilter {
                    label: "exact",
                    specific: true,
                    catch_all: false,
                    panic_sync: false,
                    panic_async: false,
                }),
                GlobalFilterBinding::new(TestFilter {
                    label: "late-catch-all",
                    specific: false,
                    catch_all: true,
                    panic_sync: false,
                    panic_async: false,
                }),
            ],
            &[],
            &[],
            &request,
        )
        .await;
        assert_eq!(exact.headers().get("x-filter-label").unwrap(), "exact");
        assert_eq!(exact.body().as_bytes(), b"exact");

        let latest_catch_all = handle_http_exception(
            HttpException::bad_request("bad"),
            &[
                GlobalFilterBinding::new(TestFilter {
                    label: "first",
                    specific: false,
                    catch_all: true,
                    panic_sync: false,
                    panic_async: false,
                }),
                GlobalFilterBinding::new(TestFilter {
                    label: "second",
                    specific: false,
                    catch_all: true,
                    panic_sync: false,
                    panic_async: false,
                }),
            ],
            &[],
            &[],
            &request,
        )
        .await;
        assert_eq!(
            latest_catch_all.headers().get("x-filter-label").unwrap(),
            "second"
        );
        assert_eq!(latest_catch_all.body().as_bytes(), b"second");
    }

    #[tokio::test]
    async fn exception_filter_panics_fall_back_to_internal_error_shape() {
        let request = NivasaRequest::new(Method::GET, "/filters", Body::empty());

        for filter in [
            TestFilter {
                label: "panic-sync",
                specific: true,
                catch_all: false,
                panic_sync: true,
                panic_async: false,
            },
            TestFilter {
                label: "panic-async",
                specific: true,
                catch_all: false,
                panic_sync: false,
                panic_async: true,
            },
        ] {
            let response = handle_http_exception(
                HttpException::bad_request("bad"),
                &[GlobalFilterBinding::new(filter)],
                &[],
                &[],
                &request,
            )
            .await;

            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
            let body: serde_json::Value =
                serde_json::from_slice(&response.body().as_bytes()).unwrap();
            assert_eq!(
                body,
                json!({
                    "statusCode": 500,
                    "message": "request handler failed",
                    "error": "Internal Server Error"
                })
            );
        }
    }

    #[tokio::test]
    async fn execute_middleware_sequence_preserves_forwarded_request_before_short_circuit() {
        let request = NivasaRequest::new(Method::GET, "/users/42", Body::empty());
        let call_count = Arc::new(AtomicUsize::new(0));

        let mut middlewares = Vec::new();
        middlewares.push(Arc::new({
            let call_count = Arc::clone(&call_count);
            move |mut request: NivasaRequest, next: NextMiddleware| {
                let call_count = Arc::clone(&call_count);
                async move {
                    call_count.fetch_add(1, Ordering::SeqCst);
                    request.set_header("x-first", "done");
                    next.run(request).await
                }
            }
        }) as MiddlewareLayer);
        middlewares.push(
            Arc::new(|request: NivasaRequest, _next: NextMiddleware| async move {
                assert_eq!(
                    request
                        .header("x-first")
                        .and_then(|value| value.to_str().ok()),
                    Some("done")
                );
                NivasaResponse::text("halted")
            }) as MiddlewareLayer,
        );

        match execute_middleware_sequence(middlewares, request).await {
            MiddlewareExecution::Forwarded(_) => panic!("middleware should short-circuit"),
            MiddlewareExecution::ShortCircuited { request, response } => {
                assert_eq!(
                    request
                        .header("x-first")
                        .and_then(|value| value.to_str().ok()),
                    Some("done")
                );
                assert_eq!(response.body().as_bytes(), b"halted");
            }
        }

        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        match execute_middleware_sequence(
            Vec::new(),
            NivasaRequest::new(Method::GET, "/passthrough", Body::empty()),
        )
        .await
        {
            MiddlewareExecution::Forwarded(request) => {
                assert_eq!(request.path(), "/passthrough");
            }
            MiddlewareExecution::ShortCircuited { .. } => {
                panic!("empty middleware sequence should forward request")
            }
        }
    }

    #[test]
    fn matching_route_middlewares_and_builder_respect_exclusions() {
        let builder =
            NivasaServer::builder()
                .apply(|request: NivasaRequest, next: NextMiddleware| async move {
                    next.run(request).await
                })
                .exclude("/users/skip")
                .unwrap()
                .for_routes("/users/:id")
                .unwrap();

        assert_eq!(builder.route_middlewares.len(), 1);

        let matched = matching_route_middlewares("/users/42", &builder.route_middlewares);
        assert_eq!(matched.len(), 1);

        let excluded = matching_route_middlewares("/users/skip", &builder.route_middlewares);
        assert!(excluded.is_empty());
    }

    #[test]
    fn request_and_response_finalizers_skip_optional_headers_when_absent() {
        let response = with_request_id(NivasaResponse::text("ok"), None);
        assert!(response.headers().get(REQUEST_ID_HEADER).is_none());

        let request = attach_module_name(
            NivasaRequest::new(Method::GET, "/users", Body::empty()),
            None,
        );
        assert!(request.header(MODULE_NAME_HEADER).is_none());

        let hyper = build_cors_preflight_response(&HeaderMap::new(), None, None);
        assert_eq!(hyper.status(), StatusCode::NO_CONTENT);
        assert!(hyper.headers().is_empty());
    }
}
