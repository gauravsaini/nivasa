//! # nivasa-http
//!
//! Nivasa framework HTTP primitives.

mod pipeline;

use http::{
    header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE},
    Method, Request, Response, StatusCode, Uri,
};
use serde::{de::DeserializeOwned, Serialize};
use std::fmt;

/// Minimal response/request body abstraction for the HTTP wrapper layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    Empty,
    Text(String),
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
            Body::Json(value) => serde_json::to_vec(value).expect("JSON body must serialize"),
            Body::Bytes(bytes) => bytes.clone(),
        }
    }

    /// Consume the body and return owned bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        match self {
            Body::Empty => Vec::new(),
            Body::Text(text) => text.into_bytes(),
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

/// Errors raised when extracting values from a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestExtractError {
    MissingBody,
    InvalidBody(String),
    InvalidQuery(String),
}

impl fmt::Display for RequestExtractError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestExtractError::MissingBody => f.write_str("request body is empty"),
            RequestExtractError::InvalidBody(err) => write!(f, "invalid request body: {err}"),
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

/// Request wrapper used by the HTTP layer.
#[derive(Debug, Clone)]
pub struct NivasaRequest {
    inner: Request<Body>,
}

impl NivasaRequest {
    /// Construct a new request from parts.
    pub fn new(method: Method, uri: impl AsRef<str>, body: impl Into<Body>) -> Self {
        let inner = Request::builder()
            .method(method)
            .uri(uri.as_ref())
            .body(body.into())
            .expect("request must have a valid URI");

        Self { inner }
    }

    /// Wrap an existing HTTP request.
    pub fn from_http(inner: Request<Body>) -> Self {
        Self { inner }
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

    /// Request body.
    pub fn body(&self) -> &Body {
        self.inner.body()
    }

    /// Mutable request body.
    pub fn body_mut(&mut self) -> &mut Body {
        self.inner.body_mut()
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
            Body::Text(text) => Ok(text.clone()),
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
            Body::Text(text) => serde_json::from_str(text)
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
            Body::Text(text) => serde_json::from_str(text)
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
            return Err(RequestExtractError::InvalidQuery("missing query string".into()));
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

    /// Create an OK response from JSON.
    pub fn json(value: impl Into<serde_json::Value>) -> Self {
        Self::new(StatusCode::OK, Body::json(value))
    }

    /// Create an OK response from raw bytes.
    pub fn bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self::new(StatusCode::OK, Body::bytes(bytes))
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
                response.headers_mut().insert(
                    CONTENT_TYPE,
                    HeaderValue::from_static(content_type),
                );
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

impl IntoResponse for serde_json::Value {
    fn into_response(self) -> NivasaResponse {
        NivasaResponse::json(self)
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
