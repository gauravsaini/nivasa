use crate::{
    Body, IntoResponse, NextMiddleware, NivasaMiddleware, NivasaRequest, NivasaResponse,
    RequestPipeline,
};
use bytes::{Bytes, BytesMut};
use futures_util::FutureExt;
use http::{
    header::{
        HeaderMap, HeaderValue, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS,
        ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_ORIGIN,
        ACCESS_CONTROL_REQUEST_HEADERS, ACCESS_CONTROL_REQUEST_METHOD, ALLOW, CONTENT_TYPE, ORIGIN,
    },
    Method, Request, Response, StatusCode,
};
use http_body_util::{BodyExt, Full};
use hyper::{body::Incoming, service::service_fn};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder as AutoBuilder;
use nivasa_common::HttpException;
use nivasa_common::RequestContext;
use nivasa_filters::HttpExceptionSummary;
use nivasa_filters::{ArgumentsHost, ExceptionFilter, ExceptionFilterMetadata};
use nivasa_interceptors::{
    CallHandler, ExecutionContext as InterceptorExecutionContext, Interceptor,
};
use nivasa_routing::{
    parse_api_version_accept, parse_api_version_header, RouteDispatchError, RouteDispatchOutcome,
    RouteDispatchRegistry, RouteMethod, RoutePattern, RouteRegistryError,
};
use serde_json::json;
use std::{
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

type RouteHandler = Arc<dyn Fn(&NivasaRequest) -> NivasaResponse + Send + Sync + 'static>;
type MiddlewareLayer = Arc<dyn NivasaMiddleware + Send + Sync + 'static>;
type InterceptorLayer = Arc<dyn Interceptor<Response = NivasaResponse> + Send + Sync + 'static>;
type GlobalFilterLayer =
    Arc<dyn ExceptionFilter<HttpException, NivasaResponse> + Send + Sync + 'static>;

const REQUEST_ID_HEADER: &str = "x-request-id";

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
    handler_filters: Vec<GlobalFilterBinding>,
    controller_filters: Vec<GlobalFilterBinding>,
}

impl RouteHandlerBinding {
    fn new(handler: impl Fn(&NivasaRequest) -> NivasaResponse + Send + Sync + 'static) -> Self {
        Self {
            handler: Arc::new(handler),
            module_middlewares: Vec::new(),
            handler_filters: Vec::new(),
            controller_filters: Vec::new(),
        }
    }

    fn with_module_middlewares(mut self, middlewares: Vec<MiddlewareLayer>) -> Self {
        self.module_middlewares = middlewares;
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorsOptions {
    allowed_origins: Option<Vec<String>>,
    allowed_methods: Option<Vec<Method>>,
    allowed_headers: Option<Vec<String>>,
    allow_credentials: bool,
}

impl Default for CorsOptions {
    fn default() -> Self {
        Self {
            allowed_origins: None,
            allowed_methods: None,
            allowed_headers: None,
            allow_credentials: false,
        }
    }
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
            if allowed_origins.iter().any(|allowed| allowed == request_origin) {
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
pub struct NivasaServer {
    routes: RouteDispatchRegistry<RouteHandlerBinding>,
    middleware: Option<MiddlewareLayer>,
    module_middlewares: Vec<MiddlewareLayer>,
    route_middlewares: Vec<RouteMiddlewareBinding>,
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
pub struct NivasaServerBuilder {
    routes: RouteDispatchRegistry<RouteHandlerBinding>,
    middleware: Option<MiddlewareLayer>,
    module_middlewares: Vec<MiddlewareLayer>,
    route_middlewares: Vec<RouteMiddlewareBinding>,
    interceptors: Vec<InterceptorLayer>,
    global_filters: Vec<GlobalFilterBinding>,
    cors: Option<CorsOptions>,
    request_timeout: Option<Duration>,
    request_body_size_limit: Option<usize>,
    shutdown: Option<oneshot::Receiver<()>>,
    #[cfg(feature = "tls")]
    tls_config: Option<Arc<rustls::ServerConfig>>,
}

impl NivasaServer {
    /// Create a new server builder.
    pub fn builder() -> NivasaServerBuilder {
        NivasaServerBuilder::new()
    }

    /// Start listening for HTTP requests.
    pub async fn listen(mut self, host: impl AsRef<str>, port: u16) -> io::Result<()> {
        let addr = socket_addr(host.as_ref(), port)?;
        let listener = TcpListener::bind(addr).await?;
        let mut shutdown = shutdown_future(self.shutdown.take());
        let routes = self.routes;
        let middleware = self.middleware;
        let module_middlewares = self.module_middlewares;
        let route_middlewares = self.route_middlewares;
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
}

impl NivasaServerBuilder {
    fn new() -> Self {
        Self {
            routes: RouteDispatchRegistry::new(),
            middleware: None,
            module_middlewares: Vec::new(),
            route_middlewares: Vec::new(),
            interceptors: Vec::new(),
            global_filters: Vec::new(),
            cors: None,
            request_timeout: None,
            request_body_size_limit: None,
            shutdown: None,
            #[cfg(feature = "tls")]
            tls_config: None,
        }
    }

    /// Register a request handler for a route.
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

        self.routes.register_pattern(
            method,
            path,
            RouteHandlerBinding::new(handler).with_module_middlewares(module_middlewares),
        )?;
        Ok(self)
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

    /// Configure transport-side CORS handling with explicit origins, methods, and headers.
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

async fn serve_connection<S>(
    stream: S,
    routes: RouteDispatchRegistry<RouteHandlerBinding>,
    middleware: Option<MiddlewareLayer>,
    module_middlewares: Vec<MiddlewareLayer>,
    route_middlewares: Vec<RouteMiddlewareBinding>,
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

async fn handle_request(
    request: hyper::Request<Incoming>,
    routes: RouteDispatchRegistry<RouteHandlerBinding>,
    middleware: Option<MiddlewareLayer>,
    module_middlewares: Vec<MiddlewareLayer>,
    route_middlewares: Vec<RouteMiddlewareBinding>,
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
                None,
            ));
        }
        Err(BodyCollectionError::Invalid) => {
            return Ok(finalize_response(
                NivasaResponse::new(StatusCode::BAD_REQUEST, Body::text("invalid request body")),
                cors.as_ref(),
                request_origin.as_deref(),
                None,
            ));
        }
    };

    let body = if body.is_empty() {
        Body::empty()
    } else {
        Body::bytes(body.to_vec())
    };

    let request = NivasaRequest::from_http(Request::from_parts(parts, body));
    let request = match middleware {
        Some(middleware) => match execute_middleware(middleware, request).await {
            MiddlewareExecution::Forwarded(request) => request,
            MiddlewareExecution::ShortCircuited { request, response } => {
                let request_id = request
                    .header(REQUEST_ID_HEADER)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned);
                let mut pipeline = RequestPipeline::new(request);

                if pipeline.parse_request().is_err() || pipeline.fail_middleware().is_err() {
                    return Ok(finalize_response(
                        NivasaResponse::new(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Body::text("request pipeline middleware transition failed"),
                        ),
                        cors.as_ref(),
                        request_origin.as_deref(),
                        None,
                    ));
                }

                return Ok(finalize_response(
                    response,
                    cors.as_ref(),
                    request_origin.as_deref(),
                    request_id.as_deref(),
                ));
            }
        },
        None => request,
    };
    let request_id = request
        .header(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let mut pipeline = RequestPipeline::new(request);

    if pipeline.parse_request().is_err() {
        return Ok(finalize_response(
            NivasaResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                Body::text("request pipeline parse transition failed"),
            ),
            cors.as_ref(),
            request_origin.as_deref(),
            request_id.as_deref(),
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
            request_id.as_deref(),
        ));
    }

    let versioned_routes = versioned_routes_for_request(pipeline.request(), &routes);

    let response = match pipeline.match_route(&versioned_routes) {
        Ok(RouteDispatchOutcome::Matched(entry)) => {
            let binding = entry.value.clone();
            let handler = Arc::clone(&binding.handler);
            let request = pipeline.request().clone();
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
                        request_id.as_deref(),
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
                        request_id.as_deref(),
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
                        request_id.as_deref(),
                    ));
                }
            };
            match interceptors.is_empty() {
                false => match execute_interceptors(interceptors, request, handler).await {
                    InterceptorExecution::Completed(response) => {
                        if pipeline.pass_guards().is_err()
                            || pipeline.complete_interceptors_pre().is_err()
                            || pipeline.complete_pipes().is_err()
                            || pipeline.complete_handler().is_err()
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
                                request_id.as_deref(),
                            )
                        }
                    }
                    InterceptorExecution::ShortCircuited(response) => {
                        if pipeline.pass_guards().is_err()
                            || pipeline.fail_interceptors_pre().is_err()
                        {
                            NivasaResponse::new(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Body::text("request pipeline interceptor transition failed"),
                            )
                        } else {
                            with_request_id(
                                map_interceptor_response(response),
                                request_id.as_deref(),
                            )
                        }
                    }
                    InterceptorExecution::Error {
                        error,
                        handler_called,
                    } => {
                        let transition_failed = if pipeline.pass_guards().is_err() {
                            true
                        } else if handler_called {
                            pipeline.complete_interceptors_pre().is_err()
                                || pipeline.complete_pipes().is_err()
                                || pipeline.complete_handler().is_err()
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
                true => match tokio::task::spawn_blocking(move || (handler)(&request)).await {
                    Ok(response) => response,
                    Err(_) => NivasaResponse::new(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Body::text("request handler failed"),
                    ),
                },
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
        request_id.as_deref(),
    ))
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
            let mut request_context = RequestContext::new();
            request_context.insert_request_data(request.clone());
            let host = ArgumentsHost::new().with_request_context(request_context);
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
        None => fallback_unhandled_exception_response(&error),
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
            mapped_response = mapped_response.with_header(
                name.as_str(),
                value
                    .to_str()
                    .expect("response header value must be valid utf-8"),
            );
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
    let mut hyper_response = Response::new(Full::new(Bytes::from(response.body().as_bytes())));
    *hyper_response.status_mut() = status;
    *hyper_response.headers_mut() = response.headers().clone();
    hyper_response
}

fn with_request_id(response: NivasaResponse, request_id: Option<&str>) -> NivasaResponse {
    match request_id {
        Some(request_id) => response.with_header(REQUEST_ID_HEADER, request_id),
        None => response,
    }
}

fn finalize_response(
    mut response: NivasaResponse,
    cors: Option<&CorsOptions>,
    request_origin: Option<&str>,
    request_id: Option<&str>,
) -> Response<Full<Bytes>> {
    response = with_request_id(response, request_id);

    let mut response = build_response(response.status(), response);
    apply_cors_headers(response.headers_mut(), cors, request_origin);
    response
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
    let mut response = Response::new(Full::new(Bytes::new()));
    *response.status_mut() = StatusCode::NO_CONTENT;
    apply_cors_headers(response.headers_mut(), cors, request_origin);

    if let Some(value) = allow_methods_header_value(headers, cors) {
        response
            .headers_mut()
            .insert(ACCESS_CONTROL_ALLOW_METHODS, value);
    }

    if let Some(value) = allow_headers_header_value(headers, cors) {
        response
            .headers_mut()
            .insert(ACCESS_CONTROL_ALLOW_HEADERS, value);
    }

    if let Some(cors) = cors {
        if let Some(value) = cors.allow_credentials_header_value() {
            response
                .headers_mut()
                .insert(ACCESS_CONTROL_ALLOW_CREDENTIALS, value);
        }
    }

    response
}

fn apply_cors_headers(
    headers: &mut HeaderMap,
    cors: Option<&CorsOptions>,
    request_origin: Option<&str>,
) {
    let Some(cors) = cors else {
        return;
    };

    if let Some(value) = cors.allow_origin_header_value(request_origin) {
        headers.insert(ACCESS_CONTROL_ALLOW_ORIGIN, value);
    }

    if let Some(value) = cors.allow_credentials_header_value() {
        headers.insert(ACCESS_CONTROL_ALLOW_CREDENTIALS, value);
    }
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

                let mut sigterm = signal(SignalKind::terminate())
                    .expect("SIGTERM signal handler must be available");
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = sigterm.recv() => {}
                }
            }

            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
            }
        }),
    }
}
