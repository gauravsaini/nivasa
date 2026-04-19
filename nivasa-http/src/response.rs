use crate::{
    body::{Body, Html, Text},
    request::Json,
};
use http::{
    header::{HeaderName, HeaderValue, CONTENT_DISPOSITION, CONTENT_TYPE},
    Response, StatusCode,
};
use nivasa_common::HttpException;
use nivasa_filters::{
    ExceptionFilter, ExceptionFilterFuture, ExceptionFilterMetadata, HttpArgumentsHost,
    HttpExceptionSummary,
};
use serde::Serialize;

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
    headers: http::HeaderMap,
    body: Body,
}

impl Default for ControllerResponse {
    fn default() -> Self {
        Self {
            status: StatusCode::OK,
            headers: http::HeaderMap::new(),
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
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_ref().as_bytes()),
            HeaderValue::from_str(value.as_ref()),
        ) {
            self.headers.insert(name, value);
        }
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

        Self::bytes(bytes).with_header(CONTENT_DISPOSITION.as_str(), disposition)
    }

    /// Access the response status.
    pub fn status(&self) -> StatusCode {
        self.inner.status()
    }

    /// Access the response headers.
    pub fn headers(&self) -> &http::HeaderMap {
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
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_ref().as_bytes()),
            HeaderValue::from_str(value.as_ref()),
        ) {
            self.inner.headers_mut().insert(name, value);
        }
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
    headers: http::HeaderMap,
    body: Body,
}

impl Default for NivasaResponseBuilder {
    fn default() -> Self {
        Self {
            status: StatusCode::OK,
            headers: http::HeaderMap::new(),
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
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_ref().as_bytes()),
            HeaderValue::from_str(value.as_ref()),
        ) {
            self.headers.insert(name, value);
        }
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

impl IntoResponse for () {
    fn into_response(self) -> NivasaResponse {
        NivasaResponse::new(StatusCode::OK, Body::empty())
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
            .with_header(CONTENT_TYPE.as_str(), "text/event-stream; charset=utf-8")
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
        let fallback = serde_json::json!({
            "statusCode": status.as_u16(),
            "message": self.to_string(),
        });
        let body = serde_json::to_value(&self).unwrap_or(fallback);
        NivasaResponse::new(status, body)
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
        NivasaResponse::json(serde_json::to_value(self.0).unwrap_or_else(|_| {
            serde_json::json!({
                "error": "response serialization failed"
            })
        }))
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
