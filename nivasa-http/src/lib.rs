//! # nivasa-http
//!
//! Nivasa framework HTTP primitives.

mod pipeline;
mod server;

use http::{
    header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE},
    Method, Request, Response, StatusCode, Uri,
};
use nivasa_common::HttpException;
use nivasa_routing::RoutePathCaptures;
use serde::{de::DeserializeOwned, Serialize};
use std::fmt;

/// Minimal response/request body abstraction for the HTTP wrapper layer.
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

/// Request wrapper used by the HTTP layer.
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
    pub fn query(&self, name: impl AsRef<str>) -> Option<&str> {
        let name = name.as_ref();
        self.inner.uri().query().and_then(|query| {
            query.split('&').find_map(|pair| {
                let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
                if key == name {
                    Some(value)
                } else {
                    None
                }
            })
        })
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

        deserialize_scalar_value(raw)
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
        let Some(query) = request.uri().query() else {
            return Err(RequestExtractError::InvalidQuery(
                "missing query string".into(),
            ));
        };

        let mut values = serde_json::Map::new();
        for pair in query.split('&').filter(|segment| !segment.is_empty()) {
            let (key, raw_value) = pair.split_once('=').unwrap_or((pair, ""));
            let value = serde_json::from_str::<serde_json::Value>(raw_value)
                .unwrap_or_else(|_| serde_json::Value::String(raw_value.to_string()));
            values.insert(key.to_string(), value);
        }

        serde_json::from_value(serde_json::Value::Object(values))
            .map(Query)
            .map_err(|err| RequestExtractError::InvalidQuery(err.to_string()))
    }
}

/// Response wrapper used by the HTTP layer.
#[derive(Debug, Clone)]
pub struct NivasaResponse {
    inner: Response<Body>,
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

impl IntoResponse for NivasaResponse {
    fn into_response(self) -> NivasaResponse {
        self
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

impl IntoResponse for Vec<u8> {
    fn into_response(self) -> NivasaResponse {
        NivasaResponse::bytes(self)
    }
}

/// Buffered server-sent events response helper.
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
            .with_header(http::header::CONTENT_TYPE.as_str(), "text/event-stream; charset=utf-8")
            .with_header(http::header::CACHE_CONTROL.as_str(), "no-cache")
    }
}

/// File download response helper backed by the existing byte body surface.
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

/// Redirect response helper with common HTTP redirect statuses.
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

pub use pipeline::RequestPipeline;
pub use server::{NivasaServer, NivasaServerBuilder};

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
