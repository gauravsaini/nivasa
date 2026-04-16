use crate::Body;
use http::{
    header::{HeaderName, HeaderValue},
    HeaderMap, Method, Request, Uri,
};
use nivasa_routing::RoutePathCaptures;
use serde::de::DeserializeOwned;
use std::fmt;
use url::form_urlencoded;

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
            .unwrap_or(serde_json::Value::String(raw_value));
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
        let body = body.into();
        let method_for_builder = method.clone();
        let inner = match Request::builder()
            .method(method_for_builder)
            .uri(uri.as_ref())
            .body(body.clone())
        {
            Ok(inner) => inner,
            Err(_) => {
                let mut inner = Request::new(body);
                *inner.method_mut() = method;
                *inner.uri_mut() = Uri::from_static("/");
                inner
            }
        };

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
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_ref().as_bytes()),
            HeaderValue::from_str(value.as_ref()),
        ) {
            self.inner.headers_mut().insert(name, value);
        }
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
