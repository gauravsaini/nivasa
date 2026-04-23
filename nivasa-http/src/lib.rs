//! # nivasa-http
//!
//! HTTP wrapper primitives for Nivasa.
//!
//! Start here if you want the small wrapper layer around requests, responses,
//! upload helpers, or the higher-level server and health surfaces.
//!
//! The most common entry points are:
//! - [`Body`], [`NivasaRequest`], and [`NivasaResponse`] for wrapper-layer HTTP values
//! - [`ControllerResponse`] for the first `#[res]` runtime slice
//! - [`CorsOptions`] and [`GlobalFilterBinding`] for server wiring
//! - [`HealthCheckService`] and [`TerminusModule`] for health checks
//! - [`LoggerModule`] and [`LoggerService`] for structured logging setup
//! - [`GraphQLModule`] for the minimal GraphQL HTTP envelope and playground
//!
//! ```rust
//! use http::{Method, StatusCode};
//! use nivasa_http::{Body, NivasaRequest, NivasaResponse};
//!
//! let request = NivasaRequest::new(Method::GET, "/users?limit=10", Body::empty());
//! assert_eq!(request.path(), "/users");
//! assert_eq!(request.query("limit"), Some("10".to_string()));
//!
//! let response = NivasaResponse::new(StatusCode::OK, Body::text("ready"));
//! assert_eq!(response.status(), StatusCode::OK);
//! assert_eq!(response.body().as_bytes(), b"ready");
//! ```
//!
//! For the transport side, [`NivasaServer`] and [`CorsOptions`] cover the
//! server builder path, while [`upload`] contains the focused multipart
//! helpers.

mod body;
pub mod graphql;
mod health;
mod logging;
mod pipeline;
mod request;
mod response;
mod server;
pub mod testing;
mod throttling;
pub mod upload;

pub use body::{Body, Html, Text};
pub use graphql::{GraphQLError, GraphQLModule, GraphQLRequest, GraphQLResponse};
pub use health::{
    DatabaseHealthIndicator, DiskHealthIndicator, HealthCheckResult, HealthCheckService,
    HealthIndicator, HealthIndicatorResult, HealthStatus, HttpHealthIndicator,
    MemoryHealthIndicator, TerminusModule,
};
pub use http::header::HeaderMap;
pub use logging::{
    LogContext, LoggerFormat, LoggerInitError, LoggerModule, LoggerOptions, LoggerOptionsProvider,
    LoggerService,
};
pub use nivasa_core::module::RouteThrottleRegistration;
pub use request::{FromRequest, Json, NivasaRequest, Query, RequestExtractError};
pub use response::{
    ControllerResponse, Download, HttpExceptionFilter, IntoResponse, NivasaResponse,
    NivasaResponseBuilder, Redirect, Sse, SseEvent, StreamBody,
};
pub use server::{CorsOptions, GlobalFilterBinding};
pub use throttling::{
    InMemoryThrottlerStorage, ThrottlerGuard, ThrottlerModule, ThrottlerOptions,
    ThrottlerOptionsProvider, ThrottlerStorage,
};

use async_trait::async_trait;
#[cfg(feature = "compression-brotli")]
use brotli::CompressorWriter;
#[cfg(feature = "compression-deflate")]
use flate2::write::DeflateEncoder;
#[cfg(feature = "compression-gzip")]
use flate2::write::GzEncoder;
#[cfg(any(
    feature = "compression-gzip",
    feature = "compression-deflate",
    feature = "compression-brotli"
))]
use flate2::Compression;
#[cfg(any(
    feature = "compression-gzip",
    feature = "compression-deflate",
    feature = "compression-brotli"
))]
use http::header::HeaderValue;
use http::header::CONTENT_TYPE;
#[cfg(any(
    feature = "compression-gzip",
    feature = "compression-deflate",
    feature = "compression-brotli"
))]
use http::Response;
use nivasa_common::HttpException;
use serde::de::DeserializeOwned;
#[cfg(any(
    feature = "compression-gzip",
    feature = "compression-deflate",
    feature = "compression-brotli"
))]
use std::io::Write;
use std::{
    collections::HashMap,
    convert::Infallible,
    fmt,
    future::Future,
    pin::Pin,
    sync::{Arc, OnceLock, RwLock},
    task::{Context, Poll},
    time::Instant,
};
use tokio::sync::Mutex;
use tower::{Layer, Service};
use uuid::Uuid;

/// Shared request/response handler type used by app-shell dispatch seams.
pub type AppRouteHandler = Arc<dyn Fn(&NivasaRequest) -> NivasaResponse + Send + Sync + 'static>;

/// Captured client IP value for controller-side `#[ip]` extraction.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientIp(String);

impl ClientIp {
    /// Create a new client IP wrapper.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrow the captured IP text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the wrapper into its string value.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<&str> for ClientIp {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ClientIp {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

/// Runtime hook used by custom controller parameter extractors.
pub trait ControllerParamExtractor<T> {
    /// Extract a value from the request after SCXML route matching has run.
    fn extract(&self, request: &NivasaRequest) -> Result<T, HttpException>;
}

static CONTROLLER_ROUTE_HANDLERS: OnceLock<RwLock<HashMap<String, AppRouteHandler>>> =
    OnceLock::new();

fn controller_route_handler_registry() -> &'static RwLock<HashMap<String, AppRouteHandler>> {
    CONTROLLER_ROUTE_HANDLERS.get_or_init(|| RwLock::new(HashMap::new()))
}

fn controller_route_handler_key(path: &str, handler: &str) -> String {
    format!("{}::{}", path.trim(), handler.trim())
}

/// Register a controller route handler for later app-shell dispatch.
///
/// The registry is intentionally tiny and keyed by the resolved route path plus
/// handler name so the app shell can resolve a handler without bypassing the
/// normal SCXML-backed HTTP pipeline.
pub fn register_controller_route_handler(
    path: impl AsRef<str>,
    handler: &str,
    route_handler: AppRouteHandler,
) {
    let key = controller_route_handler_key(path.as_ref(), handler);
    let mut registry = controller_route_handler_registry()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    registry.insert(key, route_handler);
}

/// Resolve a controller route handler from the shared registry.
pub fn resolve_controller_route_handler(path: &str, handler: &str) -> Option<AppRouteHandler> {
    let key = controller_route_handler_key(path, handler);
    let registry = controller_route_handler_registry()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    registry.get(&key).cloned()
}

/// Execute a controller-style action with mutable response access.
///
/// This is intentionally narrow: it only wires up the `#[res]`-style response
/// handle and leaves later SCXML handler-execution stages for future work.
pub fn run_controller_action<F>(request: &NivasaRequest, action: F) -> NivasaResponse
where
    F: FnOnce(&NivasaRequest, &mut ControllerResponse),
{
    let mut response = ControllerResponse::new();
    action(request, &mut response);
    response.into_response()
}

/// Execute a controller-style action with a single body-shaped extracted value.
///
/// This is intentionally narrow: it supports the first body-only runtime slice
/// on top of the existing request extraction surface and assumes route matching
/// has already been driven through the request pipeline.
pub fn run_controller_action_with_body<T, F, R>(
    request: &NivasaRequest,
    action: F,
) -> NivasaResponse
where
    T: FromRequest,
    F: FnOnce(T) -> R,
    R: IntoResponse,
{
    match request.extract::<T>() {
        Ok(body) => action(body).into_response(),
        Err(error) => HttpException::bad_request(error.to_string()).into_response(),
    }
}

/// Execute a controller-style action with raw request access only.
///
/// This is intentionally narrow: it models the `#[req]`-style runtime slice
/// after route matching has already been driven through the request pipeline.
pub fn run_controller_action_with_request<F, R>(
    request: &NivasaRequest,
    action: F,
) -> NivasaResponse
where
    F: FnOnce(&NivasaRequest) -> R,
    R: IntoResponse,
{
    action(request).into_response()
}

/// Execute a controller-style action with a single typed path parameter.
///
/// This is intentionally narrow: it relies on route matching to attach the
/// captured path parameters before controller execution begins.
pub fn run_controller_action_with_param<T, F, R>(
    request: &NivasaRequest,
    name: impl AsRef<str>,
    action: F,
) -> NivasaResponse
where
    T: DeserializeOwned,
    F: FnOnce(T) -> R,
    R: IntoResponse,
{
    match request.path_param_typed::<T>(name) {
        Ok(param) => action(param).into_response(),
        Err(error) => HttpException::bad_request(error.to_string()).into_response(),
    }
}

/// Execute a controller-style action with a full typed query DTO.
///
/// This is intentionally narrow: it exposes the `#[query]`-style runtime slice
/// after the request pipeline has already advanced through route matching.
pub fn run_controller_action_with_query<T, F, R>(
    request: &NivasaRequest,
    action: F,
) -> NivasaResponse
where
    T: DeserializeOwned,
    F: FnOnce(Query<T>) -> R,
    R: IntoResponse,
{
    match request.extract::<Query<T>>() {
        Ok(query) => action(query).into_response(),
        Err(error) => HttpException::bad_request(error.to_string()).into_response(),
    }
}

/// Execute a controller-style action with all request headers.
///
/// This is the `#[headers]` runtime slice. It stays after route matching and
/// does not introduce a lifecycle shortcut around the SCXML request pipeline.
pub fn run_controller_action_with_headers<F, R>(
    request: &NivasaRequest,
    action: F,
) -> NivasaResponse
where
    F: FnOnce(HeaderMap) -> R,
    R: IntoResponse,
{
    match request.extract::<HeaderMap>() {
        Ok(headers) => action(headers).into_response(),
        Err(error) => HttpException::bad_request(error.to_string()).into_response(),
    }
}

/// Execute a controller-style action with one typed request header.
pub fn run_controller_action_with_header<T, F, R>(
    request: &NivasaRequest,
    name: impl AsRef<str>,
    action: F,
) -> NivasaResponse
where
    T: DeserializeOwned,
    F: FnOnce(T) -> R,
    R: IntoResponse,
{
    match request.header_typed::<T>(name) {
        Ok(header) => action(header).into_response(),
        Err(error) => HttpException::bad_request(error.to_string()).into_response(),
    }
}

/// Execute a controller-style action with client IP extraction.
///
/// The helper prefers a typed [`ClientIp`] request extension, then common proxy
/// headers (`x-forwarded-for`, `x-real-ip`, and `forwarded`).
pub fn run_controller_action_with_ip<F, R>(request: &NivasaRequest, action: F) -> NivasaResponse
where
    F: FnOnce(String) -> R,
    R: IntoResponse,
{
    match controller_client_ip(request) {
        Some(ip) => action(ip).into_response(),
        None => HttpException::bad_request("request client IP is missing").into_response(),
    }
}

/// Execute a controller-style action with typed session data.
///
/// Session middleware can seed request extensions with the session payload; the
/// controller helper reads that payload after SCXML route matching.
pub fn run_controller_action_with_session<T, F, R>(
    request: &NivasaRequest,
    action: F,
) -> NivasaResponse
where
    T: Clone + Send + Sync + 'static,
    F: FnOnce(T) -> R,
    R: IntoResponse,
{
    match request.extension::<T>().cloned() {
        Some(session) => action(session).into_response(),
        None => HttpException::bad_request(format!(
            "request session data `{}` is missing",
            std::any::type_name::<T>()
        ))
        .into_response(),
    }
}

/// Execute a controller-style action with a custom parameter extractor.
pub fn run_controller_action_with_custom_param<T, E, F, R>(
    request: &NivasaRequest,
    extractor: E,
    action: F,
) -> NivasaResponse
where
    E: ControllerParamExtractor<T>,
    F: FnOnce(T) -> R,
    R: IntoResponse,
{
    match extractor.extract(request) {
        Ok(value) => action(value).into_response(),
        Err(error) => error.into_response(),
    }
}

/// Execute a controller-style action with a single uploaded file.
///
/// This is intentionally narrow: multipart parsing still happens in a focused
/// upload helper after route matching has completed, rather than inside the
/// SCXML-driven request pipeline itself.
pub fn run_controller_action_with_file<F, R>(
    request: &NivasaRequest,
    interceptor: &upload::FileInterceptor,
    action: F,
) -> NivasaResponse
where
    F: FnOnce(upload::UploadedFile) -> R,
    R: IntoResponse,
{
    let Some(content_type) = request.header(CONTENT_TYPE.as_str()) else {
        return HttpException::bad_request("request is missing header `content-type`")
            .into_response();
    };

    let Ok(content_type) = content_type.to_str() else {
        return HttpException::bad_request(
            "invalid header `content-type`: header value is not valid ASCII",
        )
        .into_response();
    };

    match interceptor.extract_from_bytes(content_type, &request.body().as_bytes()) {
        Ok(file) => action(file).into_response(),
        Err(error) => HttpException::bad_request(error.to_string()).into_response(),
    }
}

/// Execute a controller-style action with multiple uploaded files.
///
/// This keeps multipart parsing in the upload helper layer and assumes the
/// SCXML-driven request pipeline has already advanced through route matching.
pub fn run_controller_action_with_files<F, R>(
    request: &NivasaRequest,
    interceptor: &upload::FilesInterceptor,
    action: F,
) -> NivasaResponse
where
    F: FnOnce(Vec<upload::UploadedFile>) -> R,
    R: IntoResponse,
{
    let Some(content_type) = request.header(CONTENT_TYPE.as_str()) else {
        return HttpException::bad_request("request is missing header `content-type`")
            .into_response();
    };

    let Ok(content_type) = content_type.to_str() else {
        return HttpException::bad_request(
            "invalid header `content-type`: header value is not valid ASCII",
        )
        .into_response();
    };

    match interceptor.extract_from_bytes(content_type, &request.body().as_bytes()) {
        Ok(files) => action(files).into_response(),
        Err(error) => HttpException::bad_request(error.to_string()).into_response(),
    }
}

fn controller_client_ip(request: &NivasaRequest) -> Option<String> {
    request
        .extension::<ClientIp>()
        .map(|ip| ip.as_str().trim().to_string())
        .filter(|ip| !ip.is_empty())
        .or_else(|| header_first_csv_value(request, "x-forwarded-for"))
        .or_else(|| header_trimmed_value(request, "x-real-ip"))
        .or_else(|| forwarded_for_value(request))
}

fn header_trimmed_value(request: &NivasaRequest, name: &str) -> Option<String> {
    request
        .header(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn header_first_csv_value(request: &NivasaRequest, name: &str) -> Option<String> {
    header_trimmed_value(request, name).and_then(|value| {
        value
            .split(',')
            .map(str::trim)
            .find(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn forwarded_for_value(request: &NivasaRequest) -> Option<String> {
    header_trimmed_value(request, "forwarded").and_then(|value| {
        value.split(';').find_map(|part| {
            let (name, value) = part.split_once('=')?;
            if !name.trim().eq_ignore_ascii_case("for") {
                return None;
            }

            let value = value.trim().trim_matches('"');
            (!value.is_empty()).then(|| value.to_string())
        })
    })
}

/// Normalized guard contract for one controller handler.
///
/// This keeps controller-level and handler-level metadata in one place so the
/// server can resolve guard instances and drive them through
/// `RequestPipeline::evaluate_guard(...)` without understanding macro output
/// details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControllerGuardExecutionContract<'a> {
    handler: &'a str,
    guards: Vec<&'a str>,
}

impl<'a> ControllerGuardExecutionContract<'a> {
    fn new(handler: &'a str, guards: Vec<&'a str>) -> Self {
        Self { handler, guards }
    }

    /// The controller handler this guard contract applies to.
    pub fn handler(&self) -> &'a str {
        self.handler
    }

    /// Ordered guard names to resolve and execute for this handler.
    pub fn guards(&self) -> &[&'a str] {
        &self.guards
    }
}

/// Resolve the guard metadata for one controller handler into a single
/// execution contract.
///
/// The resolved order is controller-level guards first, followed by any
/// handler-specific guards declared for `handler`.
pub fn resolve_controller_guard_execution<'a>(
    handler: &'a str,
    controller_guards: &[&'a str],
    handler_guard_metadata: &[(&'a str, Vec<&'a str>)],
) -> Option<ControllerGuardExecutionContract<'a>> {
    let mut guards = controller_guards.to_vec();

    if let Some((_, handler_guards)) = handler_guard_metadata
        .iter()
        .find(|(candidate, _)| *candidate == handler)
    {
        guards.extend(handler_guards.iter().copied());
    }

    if guards.is_empty() {
        None
    } else {
        Some(ControllerGuardExecutionContract::new(handler, guards))
    }
}

type MiddlewareFuture = Pin<Box<dyn Future<Output = NivasaResponse> + Send + 'static>>;
type MiddlewareHandler = Arc<dyn Fn(NivasaRequest) -> MiddlewareFuture + Send + Sync + 'static>;

/// Continuation handle passed to middleware implementations.
#[derive(Clone)]
pub struct NextMiddleware {
    handler: MiddlewareHandler,
}

impl fmt::Debug for NextMiddleware {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NextMiddleware").finish_non_exhaustive()
    }
}

impl NextMiddleware {
    /// Construct a continuation from an async request handler.
    pub fn new<F, Fut>(handler: F) -> Self
    where
        F: Fn(NivasaRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = NivasaResponse> + Send + 'static,
    {
        Self {
            handler: Arc::new(move |request| Box::pin(handler(request))),
        }
    }

    /// Forward the request to the next middleware or terminal handler.
    pub async fn run(&self, request: NivasaRequest) -> NivasaResponse {
        (self.handler)(request).await
    }
}

/// Adapter that turns a Tower service into a `NivasaMiddleware`.
///
/// This first slice keeps the service terminal and intentionally does not
/// involve the `next` continuation yet. That lets us prove the Tower side of
/// the bridge without widening the middleware pipeline surface.
#[derive(Clone)]
pub struct TowerServiceMiddleware<S> {
    service: TowerServiceMiddlewareInner<S>,
}

#[derive(Clone)]
enum TowerServiceMiddlewareInner<S> {
    Shared(Arc<Mutex<S>>),
    Cloned(Arc<dyn Fn() -> S + Send + Sync + 'static>),
}

impl<S> TowerServiceMiddleware<S> {
    /// Wrap a Tower service so it can be used where a `NivasaMiddleware` is expected.
    pub fn new(service: S) -> Self {
        Self {
            service: TowerServiceMiddlewareInner::Shared(Arc::new(Mutex::new(service))),
        }
    }

    /// Wrap a cloneable Tower service so each request gets its own service instance.
    pub fn new_cloneable(service: S) -> Self
    where
        S: Clone + Send + Sync + 'static,
    {
        let service = Arc::new(service);
        Self {
            service: TowerServiceMiddlewareInner::Cloned(Arc::new(move || (*service).clone())),
        }
    }
}

impl<S> fmt::Debug for TowerServiceMiddleware<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TowerServiceMiddleware")
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl<S> NivasaMiddleware for TowerServiceMiddleware<S>
where
    S: Service<NivasaRequest, Response = NivasaResponse, Error = Infallible> + Send + 'static,
    S::Future: Send + 'static,
{
    async fn use_(&self, req: NivasaRequest, _next: NextMiddleware) -> NivasaResponse {
        match &self.service {
            TowerServiceMiddlewareInner::Shared(service) => {
                let mut service = service.lock().await;
                match service.call(req).await {
                    Ok(response) => response,
                    Err(error) => match error {},
                }
            }
            TowerServiceMiddlewareInner::Cloned(service) => {
                let mut service = (service.as_ref())();
                match service.call(req).await {
                    Ok(response) => response,
                    Err(error) => match error {},
                }
            }
        }
    }
}

/// Adapter that turns a `NivasaMiddleware` into a Tower `Layer`.
#[derive(Clone)]
pub struct NivasaMiddlewareLayer<M> {
    middleware: Arc<M>,
}

impl<M> NivasaMiddlewareLayer<M> {
    /// Wrap middleware so it can be applied as a Tower layer.
    pub fn new(middleware: M) -> Self {
        Self {
            middleware: Arc::new(middleware),
        }
    }
}

impl<M> fmt::Debug for NivasaMiddlewareLayer<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NivasaMiddlewareLayer")
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct NivasaMiddlewareService<S, M> {
    service: Arc<Mutex<S>>,
    middleware: Arc<M>,
}

impl<S, M> fmt::Debug for NivasaMiddlewareService<S, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NivasaMiddlewareService")
            .finish_non_exhaustive()
    }
}

impl<S, M> Layer<S> for NivasaMiddlewareLayer<M>
where
    M: NivasaMiddleware + Send + Sync + 'static,
{
    type Service = NivasaMiddlewareService<S, M>;

    fn layer(&self, service: S) -> Self::Service {
        NivasaMiddlewareService {
            service: Arc::new(Mutex::new(service)),
            middleware: Arc::clone(&self.middleware),
        }
    }
}

impl<S, M> Service<NivasaRequest> for NivasaMiddlewareService<S, M>
where
    S: Service<NivasaRequest, Response = NivasaResponse, Error = Infallible> + Send + 'static,
    S::Future: Send + 'static,
    M: NivasaMiddleware + Send + Sync + 'static,
{
    type Response = NivasaResponse;
    type Error = Infallible;
    type Future =
        Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'static>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: NivasaRequest) -> Self::Future {
        let service = Arc::clone(&self.service);
        let middleware = Arc::clone(&self.middleware);

        Box::pin(async move {
            let next = NextMiddleware::new(move |request| {
                let service = Arc::clone(&service);

                async move {
                    let future = {
                        let mut service = service.lock().await;
                        service.call(request)
                    };

                    match future.await {
                        Ok(response) => response,
                        Err(error) => match error {},
                    }
                }
            });

            Ok(middleware.use_(req, next).await)
        })
    }
}

/// Middleware surface for structured request logging.
///
/// This stays intentionally tiny: one log event around `next.run(...)` and no
/// change to the request/response flow.
#[derive(Debug, Clone, Copy, Default)]
pub struct LoggerMiddleware;

impl LoggerMiddleware {
    /// Create a new logging middleware.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl NivasaMiddleware for LoggerMiddleware {
    async fn use_(&self, req: NivasaRequest, next: NextMiddleware) -> NivasaResponse {
        let method = req.method().clone();
        let path = req.path().to_owned();
        let request = req.clone();
        let start = Instant::now();
        let response = next.run(req).await;
        let duration = start.elapsed();
        let log_context = log_context_from_request_and_response(&request, &response);

        tracing::info!(
            request_id = %log_context.request_id.as_deref().unwrap_or(""),
            user_id = %log_context.user_id.as_deref().unwrap_or(""),
            module_name = %log_context.module_name.as_deref().unwrap_or(""),
            method = %method,
            path = %path,
            status = response.status().as_u16(),
            duration = ?duration,
            "request completed"
        );

        response
    }
}

/// Middleware surface for gzip, deflate, and brotli compression.
///
/// This stays intentionally tiny: if the request advertises a supported
/// encoding and the response has a non-empty body, the body is compressed
/// after `next.run(...)` and the standard compression headers are updated.
#[cfg(any(
    feature = "compression-gzip",
    feature = "compression-deflate",
    feature = "compression-brotli"
))]
#[derive(Debug, Clone, Copy, Default)]
pub struct CompressionMiddleware;

#[cfg(any(
    feature = "compression-gzip",
    feature = "compression-deflate",
    feature = "compression-brotli"
))]
impl CompressionMiddleware {
    /// Create a new compression middleware.
    pub fn new() -> Self {
        Self
    }
}

#[cfg(any(
    feature = "compression-gzip",
    feature = "compression-deflate",
    feature = "compression-brotli"
))]
const COMPRESSION_ACCEPT_ENCODING_HEADER: &str = "accept-encoding";
#[cfg(any(
    feature = "compression-gzip",
    feature = "compression-deflate",
    feature = "compression-brotli"
))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompressionFormat {
    #[cfg(feature = "compression-brotli")]
    Brotli,
    #[cfg(feature = "compression-gzip")]
    Gzip,
    #[cfg(feature = "compression-deflate")]
    Deflate,
}

#[cfg(any(
    feature = "compression-gzip",
    feature = "compression-deflate",
    feature = "compression-brotli"
))]
fn accepts_compression(request: &NivasaRequest) -> Option<CompressionFormat> {
    request
        .header(COMPRESSION_ACCEPT_ENCODING_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value.split(',').find_map(|encoding| {
                let encoding = encoding.trim();
                let encoding = encoding
                    .split(';')
                    .next()
                    .map(str::trim)
                    .unwrap_or(encoding);

                match encoding {
                    "br" => {
                        #[cfg(feature = "compression-brotli")]
                        {
                            Some(CompressionFormat::Brotli)
                        }
                        #[cfg(not(feature = "compression-brotli"))]
                        {
                            None
                        }
                    }
                    "gzip" => {
                        #[cfg(feature = "compression-gzip")]
                        {
                            Some(CompressionFormat::Gzip)
                        }
                        #[cfg(not(feature = "compression-gzip"))]
                        {
                            None
                        }
                    }
                    "deflate" => {
                        #[cfg(feature = "compression-deflate")]
                        {
                            Some(CompressionFormat::Deflate)
                        }
                        #[cfg(not(feature = "compression-deflate"))]
                        {
                            None
                        }
                    }
                    "*" => {
                        #[cfg(feature = "compression-brotli")]
                        {
                            Some(CompressionFormat::Brotli)
                        }
                        #[cfg(all(
                            not(feature = "compression-brotli"),
                            feature = "compression-gzip"
                        ))]
                        {
                            Some(CompressionFormat::Gzip)
                        }
                        #[cfg(all(
                            not(feature = "compression-brotli"),
                            not(feature = "compression-gzip"),
                            feature = "compression-deflate"
                        ))]
                        {
                            Some(CompressionFormat::Deflate)
                        }
                        #[cfg(all(
                            not(feature = "compression-brotli"),
                            not(feature = "compression-gzip"),
                            not(feature = "compression-deflate")
                        ))]
                        {
                            None
                        }
                    }
                    "identity" => None,
                    _ => None,
                }
            })
        })
}

#[cfg(any(
    feature = "compression-gzip",
    feature = "compression-deflate",
    feature = "compression-brotli"
))]
fn append_vary_accept_encoding(headers: &mut HeaderMap) {
    use http::header::VARY;

    let updated = match headers.get(VARY).and_then(|value| value.to_str().ok()) {
        Some(existing)
            if existing
                .split(',')
                .any(|part| part.trim().eq_ignore_ascii_case("accept-encoding")) =>
        {
            None
        }
        Some(existing) => Some(format!("{existing}, Accept-Encoding")),
        None => Some(String::from("Accept-Encoding")),
    };

    if let Some(value) = updated {
        if let Ok(value) = HeaderValue::from_str(&value) {
            headers.insert(VARY, value);
        }
    }
}

#[cfg(any(
    feature = "compression-gzip",
    feature = "compression-deflate",
    feature = "compression-brotli"
))]
fn compress_response(response: NivasaResponse, format: CompressionFormat) -> NivasaResponse {
    if response.body().is_empty() {
        return response;
    }

    let inner = response.into_inner();
    let body = inner.body().clone().into_bytes();
    let compressed = match format {
        #[cfg(feature = "compression-brotli")]
        CompressionFormat::Brotli => {
            let mut encoder = CompressorWriter::new(Vec::new(), 4096, 5, 22);
            let _ = encoder.write_all(&body);
            encoder.into_inner()
        }
        #[cfg(feature = "compression-gzip")]
        CompressionFormat::Gzip => {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            let _ = encoder.write_all(&body);
            encoder.finish().unwrap_or_default()
        }
        #[cfg(feature = "compression-deflate")]
        CompressionFormat::Deflate => {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
            let _ = encoder.write_all(&body);
            encoder.finish().unwrap_or_default()
        }
    };

    let content_encoding = match format {
        #[cfg(feature = "compression-brotli")]
        CompressionFormat::Brotli => "br",
        #[cfg(feature = "compression-gzip")]
        CompressionFormat::Gzip => "gzip",
        #[cfg(feature = "compression-deflate")]
        CompressionFormat::Deflate => "deflate",
    };

    let (mut parts, _) = inner.into_parts();
    parts.headers.remove(http::header::CONTENT_LENGTH);
    parts.headers.insert(
        http::header::CONTENT_ENCODING,
        HeaderValue::from_static(content_encoding),
    );
    append_vary_accept_encoding(&mut parts.headers);
    if let Ok(value) = HeaderValue::from_str(&compressed.len().to_string()) {
        parts.headers.insert(http::header::CONTENT_LENGTH, value);
    }

    NivasaResponse::from(Response::from_parts(parts, Body::bytes(compressed)))
}

#[cfg(any(
    feature = "compression-gzip",
    feature = "compression-deflate",
    feature = "compression-brotli"
))]
#[async_trait]
impl NivasaMiddleware for CompressionMiddleware {
    async fn use_(&self, req: NivasaRequest, next: NextMiddleware) -> NivasaResponse {
        let compression = accepts_compression(&req);
        let response = next.run(req).await;

        if let Some(format) = compression {
            compress_response(response, format)
        } else {
            response
        }
    }
}

/// Middleware surface for conservative security response headers.
#[derive(Debug, Clone, Copy, Default)]
pub struct HelmetMiddleware;

impl HelmetMiddleware {
    /// Create a new security-header middleware.
    pub fn new() -> Self {
        Self
    }
}

const HELMET_CONTENT_SECURITY_POLICY: &str =
    "default-src 'self'; base-uri 'self'; frame-ancestors 'none'";
const HELMET_REFERRER_POLICY: &str = "no-referrer";
const HELMET_STRICT_TRANSPORT_SECURITY: &str = "max-age=31536000; includeSubDomains";
const HELMET_X_CONTENT_TYPE_OPTIONS: &str = "nosniff";
const HELMET_X_FRAME_OPTIONS: &str = "DENY";

#[async_trait]
impl NivasaMiddleware for HelmetMiddleware {
    async fn use_(&self, req: NivasaRequest, next: NextMiddleware) -> NivasaResponse {
        next.run(req)
            .await
            .with_header("content-security-policy", HELMET_CONTENT_SECURITY_POLICY)
            .with_header("referrer-policy", HELMET_REFERRER_POLICY)
            .with_header(
                "strict-transport-security",
                HELMET_STRICT_TRANSPORT_SECURITY,
            )
            .with_header("x-content-type-options", HELMET_X_CONTENT_TYPE_OPTIONS)
            .with_header("x-frame-options", HELMET_X_FRAME_OPTIONS)
    }
}

/// Middleware surface for request pre-processing and delegation.
///
/// This is intentionally just the foundational trait and continuation handle.
/// Full middleware registration and SCXML-driven execution wiring land later.
#[derive(Debug, Clone, Copy, Default)]
pub struct RequestIdMiddleware;

impl RequestIdMiddleware {
    /// Create a new request-id middleware.
    pub fn new() -> Self {
        Self
    }
}

const REQUEST_ID_HEADER: &str = "x-request-id";
const USER_ID_HEADER: &str = "x-user-id";
const MODULE_NAME_HEADER: &str = "x-module-name";

fn request_id_from_request(request: &NivasaRequest) -> Option<String> {
    header_value_from_request(request, REQUEST_ID_HEADER)
}

fn header_value_from_request(request: &NivasaRequest, name: &str) -> Option<String> {
    request
        .header(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn header_value_from_response(response: &NivasaResponse, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn log_context_from_request_and_response(
    request: &NivasaRequest,
    response: &NivasaResponse,
) -> LogContext {
    let request_id = header_value_from_request(request, REQUEST_ID_HEADER)
        .or_else(|| header_value_from_response(response, REQUEST_ID_HEADER));
    let user_id = header_value_from_request(request, USER_ID_HEADER)
        .or_else(|| header_value_from_response(response, USER_ID_HEADER));
    let module_name = header_value_from_request(request, MODULE_NAME_HEADER)
        .or_else(|| header_value_from_response(response, MODULE_NAME_HEADER));

    let mut context = LogContext::new();
    if let Some(request_id) = request_id {
        context = context.with_request_id(request_id);
    }
    if let Some(user_id) = user_id {
        context = context.with_user_id(user_id);
    }
    if let Some(module_name) = module_name {
        context = context.with_module_name(module_name);
    }

    context
}

fn resolve_request_id(request: &NivasaRequest) -> String {
    request_id_from_request(request).unwrap_or_else(|| Uuid::new_v4().to_string())
}

#[async_trait]
pub trait NivasaMiddleware: Send + Sync {
    async fn use_(&self, req: NivasaRequest, next: NextMiddleware) -> NivasaResponse;
}

#[async_trait]
impl<F, Fut> NivasaMiddleware for F
where
    F: Fn(NivasaRequest, NextMiddleware) -> Fut + Send + Sync,
    Fut: Future<Output = NivasaResponse> + Send + 'static,
{
    async fn use_(&self, req: NivasaRequest, next: NextMiddleware) -> NivasaResponse {
        (self)(req, next).await
    }
}

#[async_trait]
impl NivasaMiddleware for RequestIdMiddleware {
    async fn use_(&self, mut req: NivasaRequest, next: NextMiddleware) -> NivasaResponse {
        let request_id = resolve_request_id(&req);
        req.set_header(REQUEST_ID_HEADER, &request_id);

        next.run(req)
            .await
            .with_header(REQUEST_ID_HEADER, request_id)
    }
}

pub use pipeline::{GuardExecutionOutcome, RequestPipeline};
pub use server::{NivasaServer, NivasaServerBuilder};
pub use testing::{TestClient, TestResponse};
pub use upload::UploadedFile;

#[cfg(debug_assertions)]
pub mod debug {
    use nivasa_statechart::StatechartSnapshot;

    pub const STATECHART_PATH: &str = "/_nivasa/statechart";
    pub const STATECHART_SCXML_PATH: &str = "/_nivasa/statechart/scxml";
    pub const STATECHART_TRANSITIONS_PATH: &str = "/_nivasa/statechart/transitions";

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct DebugEndpointResponse {
        pub status: u16,
        pub content_type: &'static str,
        pub body: String,
    }

    pub fn handle_statechart_debug_request(
        path: &str,
        snapshot: &StatechartSnapshot,
    ) -> Option<DebugEndpointResponse> {
        match path {
            STATECHART_PATH => Some(DebugEndpointResponse {
                status: 200,
                content_type: "application/json",
                body: match serde_json::to_string_pretty(snapshot) {
                    Ok(body) => body,
                    Err(error) => panic!("snapshot must serialize: {error}"),
                },
            }),
            STATECHART_SCXML_PATH => Some(DebugEndpointResponse {
                status: 200,
                content_type: "application/xml",
                body: snapshot.raw_scxml.clone().unwrap_or_default(),
            }),
            STATECHART_TRANSITIONS_PATH => Some(DebugEndpointResponse {
                status: 200,
                content_type: "application/json",
                body: match serde_json::to_string_pretty(&snapshot.recent_transitions) {
                    Ok(body) => body,
                    Err(error) => panic!("transition log must serialize: {error}"),
                },
            }),
            _ => None,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use nivasa_statechart::{TransitionKind, TransitionRecord};

        fn snapshot() -> StatechartSnapshot {
            StatechartSnapshot {
                statechart_name: "Demo".to_string(),
                current_state: "Running".to_string(),
                scxml_hash: "sha256:demo".to_string(),
                raw_scxml: Some("<scxml/>".to_string()),
                recent_transitions: vec![TransitionRecord {
                    kind: TransitionKind::Valid,
                    from: "Idle".to_string(),
                    event: "Start".to_string(),
                    to: Some("Running".to_string()),
                    valid_events: Vec::new(),
                }],
            }
        }

        #[test]
        fn snapshot_endpoint_returns_json() {
            let response = match handle_statechart_debug_request(STATECHART_PATH, &snapshot()) {
                Some(response) => response,
                None => panic!("snapshot endpoint must exist"),
            };
            assert_eq!(response.status, 200);
            assert_eq!(response.content_type, "application/json");
            assert!(response.body.contains("\"current_state\": \"Running\""));
        }

        #[test]
        fn scxml_endpoint_returns_raw_document() {
            let response = match handle_statechart_debug_request(STATECHART_SCXML_PATH, &snapshot())
            {
                Some(response) => response,
                None => panic!("scxml endpoint must exist"),
            };
            assert_eq!(response.content_type, "application/xml");
            assert_eq!(response.body, "<scxml/>");
        }

        #[test]
        fn transitions_endpoint_returns_json() {
            let response =
                match handle_statechart_debug_request(STATECHART_TRANSITIONS_PATH, &snapshot()) {
                    Some(response) => response,
                    None => panic!("transitions endpoint must exist"),
                };
            assert_eq!(response.content_type, "application/json");
            assert!(response.body.contains("\"event\": \"Start\""));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_controller_guard_execution, ControllerGuardExecutionContract};

    #[test]
    fn resolves_controller_and_handler_guards_into_one_contract() {
        let contract = resolve_controller_guard_execution(
            "list",
            &["ControllerGuard"],
            &[("list", vec!["AuthGuard", "AuditGuard"])],
        );

        assert_eq!(
            contract,
            Some(ControllerGuardExecutionContract {
                handler: "list",
                guards: vec!["ControllerGuard", "AuthGuard", "AuditGuard"],
            })
        );
    }

    #[test]
    fn resolves_controller_only_guards_for_handlers_without_specific_metadata() {
        let contract =
            resolve_controller_guard_execution("show", &["ControllerGuard"], &[("list", vec![])]);

        assert_eq!(
            contract,
            Some(ControllerGuardExecutionContract {
                handler: "show",
                guards: vec!["ControllerGuard"],
            })
        );
    }

    #[test]
    fn returns_none_when_no_controller_or_handler_guards_exist() {
        let contract = resolve_controller_guard_execution("show", &[], &[]);

        assert_eq!(contract, None);
    }
}
