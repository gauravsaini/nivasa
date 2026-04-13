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

mod health;
mod logging;
mod pipeline;
mod server;
pub mod testing;
pub mod upload;

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
pub use server::{CorsOptions, GlobalFilterBinding};

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
use http::{
    header::{HeaderName, HeaderValue, CONTENT_TYPE},
    Method, Request, Response, StatusCode, Uri,
};
use nivasa_common::HttpException;
use nivasa_filters::{
    ExceptionFilter, ExceptionFilterFuture, ExceptionFilterMetadata, HttpArgumentsHost,
    HttpExceptionSummary,
};
use nivasa_routing::RoutePathCaptures;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    convert::Infallible,
    fmt,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Instant,
};
use tokio::sync::Mutex;
use tower::{Layer, Service};
use url::form_urlencoded;
use uuid::Uuid;

/// Minimal response/request body abstraction for the HTTP wrapper layer.
///
/// ```rust
/// use nivasa_http::Body;
///
/// let body = Body::text("hello");
/// assert_eq!(body.as_bytes(), b"hello");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    Empty,
    Text(String),
    Html(String),
    Json(serde_json::Value),
    Bytes(Vec<u8>),
}

impl Body {
    /// Create an empty body.
    pub fn empty() -> Self {
        Self::Empty
    }

    /// Create a UTF-8 text body.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    /// Create an HTML body.
    pub fn html(html: impl Into<String>) -> Self {
        Self::Html(html.into())
    }

    /// Create a JSON body.
    pub fn json(value: impl Into<serde_json::Value>) -> Self {
        Self::Json(value.into())
    }

    /// Create a raw byte body.
    pub fn bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self::Bytes(bytes.into())
    }

    /// The default content type for this body, if one is known.
    fn content_type(&self) -> Option<&'static str> {
        match self {
            Body::Empty => None,
            Body::Text(_) => Some("text/plain; charset=utf-8"),
            Body::Html(_) => Some("text/html; charset=utf-8"),
            Body::Json(_) => Some("application/json"),
            Body::Bytes(_) => Some("application/octet-stream"),
        }
    }

    /// Whether the body is empty.
    pub fn is_empty(&self) -> bool {
        matches!(self, Body::Empty)
    }

    /// Borrow the body as bytes when possible.
    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            Body::Empty => Vec::new(),
            Body::Text(text) => text.as_bytes().to_vec(),
            Body::Html(html) => html.as_bytes().to_vec(),
            Body::Json(value) => serde_json::to_vec(value).expect("JSON body must serialize"),
            Body::Bytes(bytes) => bytes.clone(),
        }
    }

    /// Consume the body and return owned bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        match self {
            Body::Empty => Vec::new(),
            Body::Text(text) => text.into_bytes(),
            Body::Html(html) => html.into_bytes(),
            Body::Json(value) => serde_json::to_vec(&value).expect("JSON body must serialize"),
            Body::Bytes(bytes) => bytes,
        }
    }
}

impl Default for Body {
    fn default() -> Self {
        Self::Empty
    }
}

impl From<&str> for Body {
    fn from(value: &str) -> Self {
        Self::text(value)
    }
}

impl From<String> for Body {
    fn from(value: String) -> Self {
        Self::text(value)
    }
}

/// Explicit text body wrapper for response conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Text<T>(pub T);

impl<T> Text<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> From<Text<T>> for Body
where
    T: Into<String>,
{
    fn from(value: Text<T>) -> Self {
        Body::text(value.0.into())
    }
}

/// Explicit HTML body wrapper for response conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Html<T>(pub T);

impl<T> Html<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> From<Html<T>> for Body
where
    T: Into<String>,
{
    fn from(value: Html<T>) -> Self {
        Body::html(value.0.into())
    }
}

impl From<Vec<u8>> for Body {
    fn from(value: Vec<u8>) -> Self {
        Self::bytes(value)
    }
}

impl From<&[u8]> for Body {
    fn from(value: &[u8]) -> Self {
        Self::bytes(value.to_vec())
    }
}

impl From<serde_json::Value> for Body {
    fn from(value: serde_json::Value) -> Self {
        Self::json(value)
    }
}

fn sanitize_sse_single_line(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '\r' | '\n' => ' ',
            other => other,
        })
        .collect()
}

fn push_sse_field(body: &mut String, name: &str, value: &str) {
    body.push_str(name);
    body.push_str(value);
    body.push('\n');
}

fn push_sse_multiline_field(body: &mut String, name: &str, value: &str) {
    for line in value.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        body.push_str(name);
        body.push_str(line);
        body.push('\n');
    }
}

fn escape_content_disposition_filename(filename: &str) -> String {
    let mut escaped = String::with_capacity(filename.len());
    for ch in filename.chars() {
        if matches!(ch, '\\' | '"') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

/// Errors raised when extracting values from a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestExtractError {
    MissingBody,
    MissingPathParameters,
    MissingPathParameter { name: String },
    MissingQueryParameter { name: String },
    MissingHeader { name: String },
    InvalidBody(String),
    InvalidPathParameter { name: String, error: String },
    InvalidQueryParameter { name: String, error: String },
    InvalidHeader { name: String, error: String },
    InvalidQuery(String),
}

impl fmt::Display for RequestExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestExtractError::MissingBody => f.write_str("request body is empty"),
            RequestExtractError::MissingPathParameters => {
                f.write_str("request has no captured path parameters")
            }
            RequestExtractError::MissingPathParameter { name } => {
                write!(f, "request is missing path parameter `{name}`")
            }
            RequestExtractError::MissingQueryParameter { name } => {
                write!(f, "request is missing query parameter `{name}`")
            }
            RequestExtractError::MissingHeader { name } => {
                write!(f, "request is missing header `{name}`")
            }
            RequestExtractError::InvalidBody(err) => write!(f, "invalid request body: {err}"),
            RequestExtractError::InvalidPathParameter { name, error } => {
                write!(f, "invalid path parameter `{name}`: {error}")
            }
            RequestExtractError::InvalidQueryParameter { name, error } => {
                write!(f, "invalid query parameter `{name}`: {error}")
            }
            RequestExtractError::InvalidHeader { name, error } => {
                write!(f, "invalid header `{name}`: {error}")
            }
            RequestExtractError::InvalidQuery(err) => write!(f, "invalid query string: {err}"),
        }
    }
}

impl std::error::Error for RequestExtractError {}

/// Values that can be extracted from a request.
pub trait FromRequest: Sized {
    fn from_request(request: &NivasaRequest) -> Result<Self, RequestExtractError>;
}

/// Query-string wrapper for typed extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query<T>(pub T);

impl<T> Query<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

/// JSON body wrapper for typed extraction and response conversion.
#[derive(Debug, Clone, PartialEq)]
pub struct Json<T>(pub T);

impl<T> Json<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

fn deserialize_path_value<T>(raw: &str) -> Result<T, String>
where
    T: DeserializeOwned,
{
    serde_json::from_str(raw)
        .or_else(|_| serde_json::from_value(serde_json::Value::String(raw.to_string())))
        .map_err(|err| err.to_string())
}

fn deserialize_scalar_value<T>(raw: &str) -> Result<T, String>
where
    T: DeserializeOwned,
{
    deserialize_path_value(raw)
}

fn query_pairs(uri: &Uri) -> impl Iterator<Item = (String, String)> + '_ {
    uri.query()
        .into_iter()
        .flat_map(|query| form_urlencoded::parse(query.as_bytes()))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
}

fn query_values(uri: &Uri) -> serde_json::Map<String, serde_json::Value> {
    let mut values = serde_json::Map::new();

    for (key, raw_value) in query_pairs(uri) {
        let value = serde_json::from_str::<serde_json::Value>(&raw_value)
            .unwrap_or_else(|_| serde_json::Value::String(raw_value));
        values.insert(key, value);
    }

    values
}

/// Request wrapper used by the HTTP layer.
///
/// ```rust
/// use http::Method;
/// use nivasa_http::{Body, NivasaRequest};
///
/// let request = NivasaRequest::new(Method::GET, "/users?limit=10", Body::empty());
/// assert_eq!(request.path(), "/users");
/// assert_eq!(request.query("limit"), Some("10".to_string()));
/// ```
#[derive(Debug, Clone)]
pub struct NivasaRequest {
    inner: Request<Body>,
    path_params: Option<RoutePathCaptures>,
}

impl NivasaRequest {
    /// Construct a new request from parts.
    pub fn new(method: Method, uri: impl AsRef<str>, body: impl Into<Body>) -> Self {
        let inner = Request::builder()
            .method(method)
            .uri(uri.as_ref())
            .body(body.into())
            .expect("request must have a valid URI");

        Self {
            inner,
            path_params: None,
        }
    }

    /// Wrap an existing HTTP request.
    pub fn from_http(inner: Request<Body>) -> Self {
        Self {
            inner,
            path_params: None,
        }
    }

    /// Request method.
    pub fn method(&self) -> &Method {
        self.inner.method()
    }

    /// Request URI.
    pub fn uri(&self) -> &Uri {
        self.inner.uri()
    }

    /// Normalized path portion of the URI.
    pub fn path(&self) -> &str {
        self.inner.uri().path()
    }

    /// Request headers.
    pub fn headers(&self) -> &HeaderMap {
        self.inner.headers()
    }

    /// Look up a single header by name.
    pub fn header(&self, name: impl AsRef<str>) -> Option<&HeaderValue> {
        HeaderName::from_bytes(name.as_ref().as_bytes())
            .ok()
            .and_then(|name| self.inner.headers().get(name))
    }

    /// Add or replace a header on the request.
    pub fn set_header(&mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> &mut Self {
        let name = HeaderName::from_bytes(name.as_ref().as_bytes())
            .expect("request header name must be valid");
        let value =
            HeaderValue::from_str(value.as_ref()).expect("request header value must be valid");
        self.inner.headers_mut().insert(name, value);
        self
    }

    /// Look up and coerce a single header value by name.
    pub fn header_typed<T>(&self, name: impl AsRef<str>) -> Result<T, RequestExtractError>
    where
        T: DeserializeOwned,
    {
        let name = name.as_ref().to_string();
        let Some(raw) = self.header(&name) else {
            return Err(RequestExtractError::MissingHeader { name });
        };

        let raw = raw
            .to_str()
            .map_err(|error| RequestExtractError::InvalidHeader {
                name: name.clone(),
                error: error.to_string(),
            })?;

        deserialize_scalar_value(raw)
            .map_err(|error| RequestExtractError::InvalidHeader { name, error })
    }

    /// Look up a single query parameter by name.
    pub fn query(&self, name: impl AsRef<str>) -> Option<String> {
        let name = name.as_ref();
        query_pairs(self.inner.uri())
            .filter_map(|(key, value)| (key == name).then_some(value))
            .last()
    }

    /// Look up and coerce a single query parameter by name.
    pub fn query_typed<T>(&self, name: impl AsRef<str>) -> Result<T, RequestExtractError>
    where
        T: DeserializeOwned,
    {
        let name = name.as_ref().to_string();
        let Some(raw) = self.query(&name) else {
            return Err(RequestExtractError::MissingQueryParameter { name });
        };

        deserialize_scalar_value(&raw)
            .map_err(|error| RequestExtractError::InvalidQueryParameter { name, error })
    }

    /// Request body.
    pub fn body(&self) -> &Body {
        self.inner.body()
    }

    /// Mutable request body.
    pub fn body_mut(&mut self) -> &mut Body {
        self.inner.body_mut()
    }

    /// Attach captured path parameters to this request.
    pub fn set_path_params(&mut self, path_params: RoutePathCaptures) {
        self.path_params = Some(path_params);
    }

    /// Clear any attached path parameters.
    pub fn clear_path_params(&mut self) {
        self.path_params = None;
    }

    /// Borrow the captured path parameters, if any.
    pub fn path_params(&self) -> Option<&RoutePathCaptures> {
        self.path_params.as_ref()
    }

    /// Look up a captured path parameter by name.
    pub fn path_param(&self, name: impl AsRef<str>) -> Option<&str> {
        self.path_params
            .as_ref()
            .and_then(|captures| captures.get(name.as_ref()))
    }

    /// Look up and coerce a captured path parameter by name.
    pub fn path_param_typed<T>(&self, name: impl AsRef<str>) -> Result<T, RequestExtractError>
    where
        T: DeserializeOwned,
    {
        let name = name.as_ref().to_string();
        let Some(raw) = self.path_param(&name) else {
            return Err(RequestExtractError::MissingPathParameter { name });
        };

        deserialize_path_value(raw)
            .map_err(|error| RequestExtractError::InvalidPathParameter { name, error })
    }

    /// Consume the wrapper and return the inner request.
    pub fn into_inner(self) -> Request<Body> {
        self.inner
    }

    /// Break the wrapper into request parts and body.
    pub fn into_parts(self) -> (http::request::Parts, Body) {
        self.inner.into_parts()
    }

    /// Extract a typed value from this request.
    pub fn extract<T: FromRequest>(&self) -> Result<T, RequestExtractError> {
        T::from_request(self)
    }
}

impl From<Request<Body>> for NivasaRequest {
    fn from(inner: Request<Body>) -> Self {
        Self::from_http(inner)
    }
}

impl From<NivasaRequest> for Request<Body> {
    fn from(value: NivasaRequest) -> Self {
        value.into_inner()
    }
}

impl FromRequest for NivasaRequest {
    fn from_request(request: &NivasaRequest) -> Result<Self, RequestExtractError> {
        Ok(request.clone())
    }
}

impl FromRequest for RoutePathCaptures {
    fn from_request(request: &NivasaRequest) -> Result<Self, RequestExtractError> {
        request
            .path_params()
            .cloned()
            .ok_or(RequestExtractError::MissingPathParameters)
    }
}

impl FromRequest for HeaderMap {
    fn from_request(request: &NivasaRequest) -> Result<Self, RequestExtractError> {
        Ok(request.headers().clone())
    }
}

impl FromRequest for Body {
    fn from_request(request: &NivasaRequest) -> Result<Self, RequestExtractError> {
        Ok(request.body().clone())
    }
}

impl FromRequest for Vec<u8> {
    fn from_request(request: &NivasaRequest) -> Result<Self, RequestExtractError> {
        Ok(request.body().clone().into_bytes())
    }
}

impl FromRequest for String {
    fn from_request(request: &NivasaRequest) -> Result<Self, RequestExtractError> {
        match request.body() {
            Body::Empty => Ok(String::new()),
            Body::Text(text) | Body::Html(text) => Ok(text.clone()),
            Body::Json(value) => serde_json::to_string(value)
                .map_err(|err| RequestExtractError::InvalidBody(err.to_string())),
            Body::Bytes(bytes) => String::from_utf8(bytes.clone())
                .map_err(|err| RequestExtractError::InvalidBody(err.to_string())),
        }
    }
}

impl FromRequest for serde_json::Value {
    fn from_request(request: &NivasaRequest) -> Result<Self, RequestExtractError> {
        match request.body() {
            Body::Empty => Err(RequestExtractError::MissingBody),
            Body::Text(text) | Body::Html(text) => serde_json::from_str(text)
                .map_err(|err| RequestExtractError::InvalidBody(err.to_string())),
            Body::Json(value) => Ok(value.clone()),
            Body::Bytes(bytes) => serde_json::from_slice(bytes)
                .map_err(|err| RequestExtractError::InvalidBody(err.to_string())),
        }
    }
}

impl<T> FromRequest for Json<T>
where
    T: DeserializeOwned,
{
    fn from_request(request: &NivasaRequest) -> Result<Self, RequestExtractError> {
        let value = match request.body() {
            Body::Empty => return Err(RequestExtractError::MissingBody),
            Body::Text(text) | Body::Html(text) => serde_json::from_str(text)
                .map_err(|err| RequestExtractError::InvalidBody(err.to_string()))?,
            Body::Json(value) => value.clone(),
            Body::Bytes(bytes) => serde_json::from_slice(bytes)
                .map_err(|err| RequestExtractError::InvalidBody(err.to_string()))?,
        };

        serde_json::from_value(value)
            .map(Json)
            .map_err(|err| RequestExtractError::InvalidBody(err.to_string()))
    }
}

impl<T> FromRequest for Query<T>
where
    T: DeserializeOwned,
{
    fn from_request(request: &NivasaRequest) -> Result<Self, RequestExtractError> {
        let value = serde_json::Value::Object(query_values(request.uri()));
        let payload = serde_json::to_vec(&value)
            .map_err(|err| RequestExtractError::InvalidQuery(err.to_string()))?;
        let mut deserializer = serde_json::Deserializer::from_slice(&payload);

        serde_path_to_error::deserialize(&mut deserializer)
            .map(Query)
            .map_err(|err| {
                let path = err.path().to_string();
                let error = err.into_inner();
                let message = if path.is_empty() {
                    error.to_string()
                } else {
                    format!("field `{path}`: {error}")
                };
                RequestExtractError::InvalidQuery(message)
            })
    }
}

/// Response wrapper used by the HTTP layer.
///
/// ```rust
/// use http::StatusCode;
/// use nivasa_http::NivasaResponse;
///
/// let response = NivasaResponse::new(StatusCode::CREATED, "saved");
/// assert_eq!(response.status(), StatusCode::CREATED);
/// assert_eq!(response.body().as_bytes(), b"saved");
/// ```
#[derive(Debug, Clone)]
pub struct NivasaResponse {
    inner: Response<Body>,
}

/// Mutable controller response handle for the first `#[res]` runtime slice.
///
/// ```rust
/// use http::StatusCode;
/// use nivasa_http::ControllerResponse;
///
/// let mut response = ControllerResponse::new();
/// response
///     .status(StatusCode::NO_CONTENT)
///     .header("x-trace-id", "abc123")
///     .body("done");
/// ```
#[derive(Debug, Clone)]
pub struct ControllerResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Body,
}

impl Default for ControllerResponse {
    fn default() -> Self {
        Self {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Body::empty(),
        }
    }
}

impl ControllerResponse {
    /// Create a new mutable controller response handle.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the response status code.
    pub fn status(&mut self, status: StatusCode) -> &mut Self {
        self.status = status;
        self
    }

    /// Add or replace a response header.
    pub fn header(&mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> &mut Self {
        let name = HeaderName::from_bytes(name.as_ref().as_bytes())
            .expect("response header name must be valid");
        let value =
            HeaderValue::from_str(value.as_ref()).expect("response header value must be valid");
        self.headers.insert(name, value);
        self
    }

    /// Replace the current response body.
    pub fn body(&mut self, body: impl Into<Body>) -> &mut Self {
        self.body = body.into();
        self
    }

    /// Set a text response body.
    pub fn text(&mut self, text: impl Into<String>) -> &mut Self {
        self.body(Body::text(text))
    }

    /// Set an HTML response body.
    pub fn html(&mut self, html: impl Into<String>) -> &mut Self {
        self.body(Body::html(html))
    }

    /// Set a JSON response body.
    pub fn json(&mut self, value: impl Into<serde_json::Value>) -> &mut Self {
        self.body(Body::json(value))
    }

    /// Set a raw byte response body.
    pub fn bytes(&mut self, bytes: impl Into<Vec<u8>>) -> &mut Self {
        self.body(Body::bytes(bytes))
    }
}

impl NivasaResponse {
    /// Create a response with a status code and body.
    pub fn new(status: StatusCode, body: impl Into<Body>) -> Self {
        Self::builder().status(status).body(body)
    }

    /// Create a builder for a response.
    pub fn builder() -> NivasaResponseBuilder {
        NivasaResponseBuilder::default()
    }

    /// Create an OK response from text.
    pub fn text(text: impl Into<String>) -> Self {
        Self::new(StatusCode::OK, Body::text(text))
    }

    /// Create an OK response from HTML.
    pub fn html(html: impl Into<String>) -> Self {
        Self::new(StatusCode::OK, Body::html(html))
    }

    /// Create an OK response from JSON.
    pub fn json(value: impl Into<serde_json::Value>) -> Self {
        Self::new(StatusCode::OK, Body::json(value))
    }

    /// Create an OK response from raw bytes.
    pub fn bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self::new(StatusCode::OK, Body::bytes(bytes))
    }

    /// Create an OK attachment response from raw bytes.
    pub fn download(filename: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        let filename = filename.into();
        let disposition = format!(
            "attachment; filename=\"{}\"",
            escape_content_disposition_filename(&filename)
        );

        Self::bytes(bytes).with_header(http::header::CONTENT_DISPOSITION.as_str(), disposition)
    }

    /// Access the response status.
    pub fn status(&self) -> StatusCode {
        self.inner.status()
    }

    /// Access the response headers.
    pub fn headers(&self) -> &HeaderMap {
        self.inner.headers()
    }

    /// Access the response body.
    pub fn body(&self) -> &Body {
        self.inner.body()
    }

    /// Consume the wrapper and return the HTTP response.
    pub fn into_inner(self) -> Response<Body> {
        self.inner
    }

    /// Add or replace a header on the response.
    pub fn with_header(mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        let name = HeaderName::from_bytes(name.as_ref().as_bytes())
            .expect("response header name must be valid");
        let value =
            HeaderValue::from_str(value.as_ref()).expect("response header value must be valid");
        self.inner.headers_mut().insert(name, value);
        self
    }

    /// Create a redirect response with a `Location` header.
    pub fn redirect(status: StatusCode, location: impl Into<String>) -> Self {
        let location = location.into();
        let mut response = Self::new(status, Body::empty());
        response = response.with_header("location", location);
        response
    }
}

impl From<Response<Body>> for NivasaResponse {
    fn from(inner: Response<Body>) -> Self {
        Self { inner }
    }
}

impl From<NivasaResponse> for Response<Body> {
    fn from(value: NivasaResponse) -> Self {
        value.into_inner()
    }
}

impl IntoResponse for NivasaResponseBuilder {
    fn into_response(self) -> NivasaResponse {
        self.build()
    }
}

impl IntoResponse for StatusCode {
    fn into_response(self) -> NivasaResponse {
        NivasaResponse::new(self, Body::empty())
    }
}

/// Builder for `NivasaResponse`.
///
/// ```rust
/// use http::StatusCode;
/// use nivasa_http::NivasaResponse;
///
/// let response = NivasaResponse::builder()
///     .status(StatusCode::ACCEPTED)
///     .header("x-powered-by", "nivasa")
///     .body("queued");
///
/// assert_eq!(response.status(), StatusCode::ACCEPTED);
/// ```
#[derive(Debug, Clone)]
pub struct NivasaResponseBuilder {
    status: StatusCode,
    headers: HeaderMap,
    body: Body,
}

impl Default for NivasaResponseBuilder {
    fn default() -> Self {
        Self {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Body::empty(),
        }
    }
}

impl NivasaResponseBuilder {
    /// Set the response status code.
    pub fn status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }

    /// Add or replace a response header.
    pub fn header(mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        let name = HeaderName::from_bytes(name.as_ref().as_bytes())
            .expect("response header name must be valid");
        let value =
            HeaderValue::from_str(value.as_ref()).expect("response header value must be valid");
        self.headers.insert(name, value);
        self
    }

    /// Set the body for the response.
    pub fn body(mut self, body: impl Into<Body>) -> NivasaResponse {
        self.body = body.into();
        self.build()
    }

    /// Finalize the response.
    pub fn build(self) -> NivasaResponse {
        let content_type = self.body.content_type();
        let mut response = Response::new(self.body);
        *response.status_mut() = self.status;
        *response.headers_mut() = self.headers;

        if response.headers().get(CONTENT_TYPE).is_none() {
            if let Some(content_type) = content_type {
                response
                    .headers_mut()
                    .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
            }
        }

        NivasaResponse { inner: response }
    }
}

/// Convert values into a `NivasaResponse`.
pub trait IntoResponse {
    fn into_response(self) -> NivasaResponse;
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

impl IntoResponse for NivasaResponse {
    fn into_response(self) -> NivasaResponse {
        self
    }
}

impl IntoResponse for ControllerResponse {
    fn into_response(self) -> NivasaResponse {
        let Self {
            status,
            headers,
            body,
        } = self;

        NivasaResponseBuilder {
            status,
            headers,
            body,
        }
        .build()
    }
}

impl IntoResponse for Body {
    fn into_response(self) -> NivasaResponse {
        NivasaResponse::new(StatusCode::OK, self)
    }
}

impl<T> IntoResponse for Text<T>
where
    T: Into<String>,
{
    fn into_response(self) -> NivasaResponse {
        NivasaResponse::text(self.0)
    }
}

impl<T> IntoResponse for Html<T>
where
    T: Into<String>,
{
    fn into_response(self) -> NivasaResponse {
        NivasaResponse::html(self.0)
    }
}

impl IntoResponse for String {
    fn into_response(self) -> NivasaResponse {
        NivasaResponse::text(self)
    }
}

impl IntoResponse for &str {
    fn into_response(self) -> NivasaResponse {
        NivasaResponse::text(self)
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
    service: Arc<Mutex<S>>,
}

impl<S> TowerServiceMiddleware<S> {
    /// Wrap a Tower service so it can be used where a `NivasaMiddleware` is expected.
    pub fn new(service: S) -> Self {
        Self {
            service: Arc::new(Mutex::new(service)),
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
        let mut service = self.service.lock().await;
        match service.call(req).await {
            Ok(response) => response,
            Err(error) => match error {},
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
        headers.insert(
            VARY,
            HeaderValue::from_str(&value).expect("vary header must be valid"),
        );
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
            encoder
                .write_all(&body)
                .expect("brotli compression must succeed");
            encoder.into_inner()
        }
        #[cfg(feature = "compression-gzip")]
        CompressionFormat::Gzip => {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder
                .write_all(&body)
                .expect("gzip compression must succeed");
            encoder.finish().expect("gzip compression must finish")
        }
        #[cfg(feature = "compression-deflate")]
        CompressionFormat::Deflate => {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
            encoder
                .write_all(&body)
                .expect("deflate compression must succeed");
            encoder.finish().expect("deflate compression must finish")
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
    parts.headers.insert(
        http::header::CONTENT_LENGTH,
        HeaderValue::from_str(&compressed.len().to_string())
            .expect("compressed body length must be valid"),
    );

    NivasaResponse {
        inner: Response::from_parts(parts, Body::bytes(compressed)),
    }
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

impl IntoResponse for Vec<u8> {
    fn into_response(self) -> NivasaResponse {
        NivasaResponse::bytes(self)
    }
}

fn infer_stream_content_type(chunks: &[Body]) -> Option<&'static str> {
    let mut content_type = None;

    for chunk in chunks {
        if chunk.is_empty() {
            continue;
        }

        let chunk_content_type = chunk.content_type()?;
        match content_type {
            None => content_type = Some(chunk_content_type),
            Some(existing) if existing == chunk_content_type => {}
            Some(_) => return None,
        }
    }

    content_type
}

/// Buffered streaming response helper.
///
/// ```rust
/// use nivasa_http::NivasaResponse;
///
/// let response = NivasaResponse::stream(["part one", "part two"]);
/// assert!(!response.body().is_empty());
/// ```
///
/// This collects chunked bodies into a single wrapper-layer response without
/// requiring transport-level streaming support.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamBody {
    chunks: Vec<Body>,
    content_type: Option<String>,
}

impl StreamBody {
    /// Create a streaming response from buffered chunks.
    pub fn new<I, B>(chunks: I) -> Self
    where
        I: IntoIterator<Item = B>,
        B: Into<Body>,
    {
        Self {
            chunks: chunks.into_iter().map(Into::into).collect(),
            content_type: None,
        }
    }

    /// Append an additional buffered chunk.
    pub fn push(mut self, chunk: impl Into<Body>) -> Self {
        self.chunks.push(chunk.into());
        self
    }

    /// Override the inferred `Content-Type`.
    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    fn into_parts(self) -> (Body, Option<String>) {
        let Self {
            chunks,
            content_type,
        } = self;

        let content_type = content_type
            .or_else(|| infer_stream_content_type(&chunks).map(std::borrow::ToOwned::to_owned));

        let mut body = Vec::new();
        for chunk in chunks {
            body.extend_from_slice(&chunk.as_bytes());
        }

        let body = if body.is_empty() {
            Body::empty()
        } else {
            Body::bytes(body)
        };

        (body, content_type)
    }
}

impl NivasaResponse {
    /// Create an OK streaming response from buffered chunks.
    pub fn stream<I, B>(chunks: I) -> Self
    where
        I: IntoIterator<Item = B>,
        B: Into<Body>,
    {
        StreamBody::new(chunks).into_response()
    }
}

impl IntoResponse for StreamBody {
    fn into_response(self) -> NivasaResponse {
        let (body, content_type) = self.into_parts();
        let mut response = NivasaResponse::new(StatusCode::OK, body);

        if let Some(content_type) = content_type {
            response = response.with_header(CONTENT_TYPE.as_str(), content_type);
        }

        response
    }
}

/// Buffered server-sent events response helper.
///
/// ```rust
/// use nivasa_http::{NivasaResponse, SseEvent};
///
/// let response = NivasaResponse::sse([SseEvent::data("ready").event("status")]);
/// assert_eq!(response.status(), http::StatusCode::OK);
/// ```
///
/// This frames SSE payloads into a `text/event-stream` body without introducing
/// transport-level streaming requirements in the wrapper layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    event: Option<String>,
    id: Option<String>,
    retry: Option<u64>,
    data: Vec<String>,
    comment: Vec<String>,
}

impl SseEvent {
    /// Create an event with a single `data:` field.
    pub fn data(data: impl Into<String>) -> Self {
        Self {
            event: None,
            id: None,
            retry: None,
            data: vec![data.into()],
            comment: Vec::new(),
        }
    }

    /// Create a comment-only SSE frame.
    pub fn comment(comment: impl Into<String>) -> Self {
        Self {
            event: None,
            id: None,
            retry: None,
            data: Vec::new(),
            comment: vec![comment.into()],
        }
    }

    /// Set the event name.
    pub fn event(mut self, event: impl Into<String>) -> Self {
        self.event = Some(event.into());
        self
    }

    /// Set the event identifier.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the reconnection delay, in milliseconds.
    pub fn retry(mut self, retry_ms: u64) -> Self {
        self.retry = Some(retry_ms);
        self
    }

    /// Append an additional data line to the event.
    pub fn data_line(mut self, data: impl Into<String>) -> Self {
        self.data.push(data.into());
        self
    }

    fn render(&self, body: &mut String) {
        if let Some(event) = &self.event {
            push_sse_field(body, "event: ", &sanitize_sse_single_line(event));
        }

        if let Some(id) = &self.id {
            push_sse_field(body, "id: ", &sanitize_sse_single_line(id));
        }

        if let Some(retry) = self.retry {
            push_sse_field(body, "retry: ", &retry.to_string());
        }

        for comment in &self.comment {
            push_sse_multiline_field(body, ": ", comment);
        }

        for data in &self.data {
            push_sse_multiline_field(body, "data: ", data);
        }

        body.push('\n');
    }
}

/// Buffered SSE response with one or more preframed events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sse {
    events: Vec<SseEvent>,
}

impl Sse {
    /// Create an SSE response from a sequence of events.
    pub fn new(events: impl IntoIterator<Item = SseEvent>) -> Self {
        Self {
            events: events.into_iter().collect(),
        }
    }

    /// Append an event to the buffered stream.
    pub fn push(mut self, event: SseEvent) -> Self {
        self.events.push(event);
        self
    }

    fn into_body(self) -> String {
        let mut body = String::new();
        for event in self.events {
            event.render(&mut body);
        }
        body
    }
}

impl From<SseEvent> for Sse {
    fn from(event: SseEvent) -> Self {
        Self::new([event])
    }
}

impl NivasaResponse {
    /// Create an OK server-sent events response.
    pub fn sse(events: impl IntoIterator<Item = SseEvent>) -> Self {
        Sse::new(events).into_response()
    }
}

impl IntoResponse for SseEvent {
    fn into_response(self) -> NivasaResponse {
        Sse::from(self).into_response()
    }
}

impl IntoResponse for Sse {
    fn into_response(self) -> NivasaResponse {
        NivasaResponse::new(StatusCode::OK, Body::text(self.into_body()))
            .with_header(
                http::header::CONTENT_TYPE.as_str(),
                "text/event-stream; charset=utf-8",
            )
            .with_header(http::header::CACHE_CONTROL.as_str(), "no-cache")
    }
}

/// File download response helper backed by the existing byte body surface.
///
/// ```rust
/// use nivasa_http::{Download, IntoResponse};
///
/// let response = Download::attachment("report.txt", b"contents".to_vec()).into_response();
/// assert_eq!(response.status(), http::StatusCode::OK);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    filename: String,
    bytes: Vec<u8>,
}

impl Download {
    pub fn attachment(filename: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            filename: filename.into(),
            bytes: bytes.into(),
        }
    }
}

impl IntoResponse for Download {
    fn into_response(self) -> NivasaResponse {
        NivasaResponse::download(self.filename, self.bytes)
    }
}

impl IntoResponse for serde_json::Value {
    fn into_response(self) -> NivasaResponse {
        NivasaResponse::json(self)
    }
}

impl IntoResponse for HttpException {
    fn into_response(self) -> NivasaResponse {
        let status =
            StatusCode::from_u16(self.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        NivasaResponse::new(
            status,
            serde_json::to_value(self).expect("HttpException must serialize"),
        )
    }
}

impl IntoResponse for HttpExceptionSummary {
    fn into_response(self) -> NivasaResponse {
        let status =
            StatusCode::from_u16(self.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        NivasaResponse::new(
            status,
            serde_json::json!({
                "statusCode": self.status_code,
                "message": self.message,
                "error": self.error,
            }),
        )
    }
}

/// Built-in adapter that maps any `HttpException` into the standard HTTP error body.
#[derive(Debug, Clone, Copy, Default)]
pub struct HttpExceptionFilter;

impl HttpExceptionFilter {
    pub fn new() -> Self {
        Self
    }
}

impl ExceptionFilter<HttpException, NivasaResponse> for HttpExceptionFilter {
    fn catch<'a>(
        &'a self,
        exception: HttpException,
        _host: HttpArgumentsHost,
    ) -> ExceptionFilterFuture<'a, NivasaResponse> {
        Box::pin(async move { HttpExceptionSummary::from(&exception).into_response() })
    }
}

impl ExceptionFilterMetadata for HttpExceptionFilter {
    fn is_catch_all(&self) -> bool {
        true
    }
}

/// Redirect response helper with common HTTP redirect statuses.
///
/// ```rust
/// use nivasa_http::{IntoResponse, Redirect};
///
/// let response = Redirect::temporary("/login").into_response();
/// assert_eq!(response.status(), http::StatusCode::FOUND);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirect {
    status: StatusCode,
    location: String,
}

impl Redirect {
    pub fn to(location: impl Into<String>, status: StatusCode) -> Self {
        Self {
            status,
            location: location.into(),
        }
    }

    pub fn permanent(location: impl Into<String>) -> Self {
        Self::to(location, StatusCode::MOVED_PERMANENTLY)
    }

    pub fn temporary(location: impl Into<String>) -> Self {
        Self::to(location, StatusCode::FOUND)
    }

    pub fn temporary_preserve_method(location: impl Into<String>) -> Self {
        Self::to(location, StatusCode::TEMPORARY_REDIRECT)
    }

    pub fn permanent_preserve_method(location: impl Into<String>) -> Self {
        Self::to(location, StatusCode::PERMANENT_REDIRECT)
    }
}

impl IntoResponse for Redirect {
    fn into_response(self) -> NivasaResponse {
        NivasaResponse::redirect(self.status, self.location)
    }
}

impl<T> IntoResponse for Json<T>
where
    T: Serialize,
{
    fn into_response(self) -> NivasaResponse {
        NivasaResponse::json(
            serde_json::to_value(self.0).expect("JSON response value must serialize"),
        )
    }
}

impl<T> IntoResponse for (StatusCode, T)
where
    T: IntoResponse,
{
    fn into_response(self) -> NivasaResponse {
        let (status, value) = self;
        let mut response = value.into_response();
        *response.inner.status_mut() = status;
        response
    }
}

impl<T, E> IntoResponse for Result<T, E>
where
    T: IntoResponse,
    E: IntoResponse,
{
    fn into_response(self) -> NivasaResponse {
        match self {
            Ok(value) => value.into_response(),
            Err(error) => error.into_response(),
        }
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
                body: serde_json::to_string_pretty(snapshot).expect("snapshot must serialize"),
            }),
            STATECHART_SCXML_PATH => Some(DebugEndpointResponse {
                status: 200,
                content_type: "application/xml",
                body: snapshot.raw_scxml.clone().unwrap_or_default(),
            }),
            STATECHART_TRANSITIONS_PATH => Some(DebugEndpointResponse {
                status: 200,
                content_type: "application/json",
                body: serde_json::to_string_pretty(&snapshot.recent_transitions)
                    .expect("transition log must serialize"),
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
            let response = handle_statechart_debug_request(STATECHART_PATH, &snapshot()).unwrap();
            assert_eq!(response.status, 200);
            assert_eq!(response.content_type, "application/json");
            assert!(response.body.contains("\"current_state\": \"Running\""));
        }

        #[test]
        fn scxml_endpoint_returns_raw_document() {
            let response =
                handle_statechart_debug_request(STATECHART_SCXML_PATH, &snapshot()).unwrap();
            assert_eq!(response.content_type, "application/xml");
            assert_eq!(response.body, "<scxml/>");
        }

        #[test]
        fn transitions_endpoint_returns_json() {
            let response =
                handle_statechart_debug_request(STATECHART_TRANSITIONS_PATH, &snapshot()).unwrap();
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
