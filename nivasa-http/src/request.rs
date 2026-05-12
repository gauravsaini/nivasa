use crate::Body;
use http::{
    header::{HeaderName, HeaderValue},
    HeaderMap, Method, Request, Uri,
};
use nivasa_routing::RoutePathCaptures;
use serde::de::DeserializeOwned;
use std::{fmt, net::IpAddr};
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
    MissingExtension { type_name: &'static str },
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
            RequestExtractError::MissingExtension { type_name } => {
                write!(f, "request is missing extension `{type_name}`")
            }
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

    /// Borrow request extensions.
    pub fn extensions(&self) -> &http::Extensions {
        self.inner.extensions()
    }

    /// Borrow request extensions mutably.
    pub fn extensions_mut(&mut self) -> &mut http::Extensions {
        self.inner.extensions_mut()
    }

    /// Insert a typed request extension.
    pub fn insert_extension<T>(&mut self, value: T) -> Option<T>
    where
        T: Clone + Send + Sync + 'static,
    {
        self.inner.extensions_mut().insert(value)
    }

    /// Borrow a typed request extension.
    pub fn extension<T>(&self) -> Option<&T>
    where
        T: Send + Sync + 'static,
    {
        self.inner.extensions().get::<T>()
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

impl FromRequest for IpAddr {
    fn from_request(request: &NivasaRequest) -> Result<Self, RequestExtractError> {
        request
            .extension::<IpAddr>()
            .copied()
            .ok_or(RequestExtractError::MissingExtension {
                type_name: std::any::type_name::<IpAddr>(),
            })
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


#[cfg(test)]
mod tests {
    use super::*;
    use http::Method;
    use serde::Deserialize;

    fn make_req(method: Method, uri: &str, body: Body) -> NivasaRequest {
        NivasaRequest::new(method, uri, body)
    }

    // ── RequestExtractError Display ───────────────────────────────────────────

    #[test]
    fn display_missing_body() {
        let msg = RequestExtractError::MissingBody.to_string();
        assert!(msg.contains("body"), "unexpected: {msg}");
    }

    #[test]
    fn display_missing_path_parameters() {
        let msg = RequestExtractError::MissingPathParameters.to_string();
        assert!(msg.contains("path"), "unexpected: {msg}");
    }

    #[test]
    fn display_missing_path_parameter() {
        let msg = RequestExtractError::MissingPathParameter {
            name: "id".to_string(),
        }
        .to_string();
        assert!(msg.contains("id"), "unexpected: {msg}");
    }

    #[test]
    fn display_missing_query_parameter() {
        let msg = RequestExtractError::MissingQueryParameter {
            name: "limit".to_string(),
        }
        .to_string();
        assert!(msg.contains("limit"), "unexpected: {msg}");
    }

    #[test]
    fn display_missing_header() {
        let msg = RequestExtractError::MissingHeader {
            name: "x-token".to_string(),
        }
        .to_string();
        assert!(msg.contains("x-token"), "unexpected: {msg}");
    }

    #[test]
    fn display_invalid_body() {
        let msg = RequestExtractError::InvalidBody("bad json".to_string()).to_string();
        assert!(msg.contains("bad json"), "unexpected: {msg}");
    }

    #[test]
    fn display_invalid_path_parameter() {
        let msg = RequestExtractError::InvalidPathParameter {
            name: "id".to_string(),
            error: "not a number".to_string(),
        }
        .to_string();
        assert!(msg.contains("id"), "unexpected: {msg}");
        assert!(msg.contains("not a number"), "unexpected: {msg}");
    }

    #[test]
    fn display_invalid_query_parameter() {
        let msg = RequestExtractError::InvalidQueryParameter {
            name: "page".to_string(),
            error: "NaN".to_string(),
        }
        .to_string();
        assert!(msg.contains("page"), "unexpected: {msg}");
    }

    #[test]
    fn display_invalid_header() {
        let msg = RequestExtractError::InvalidHeader {
            name: "x-count".to_string(),
            error: "not int".to_string(),
        }
        .to_string();
        assert!(msg.contains("x-count"), "unexpected: {msg}");
    }

    #[test]
    fn display_invalid_query() {
        let msg = RequestExtractError::InvalidQuery("decode error".to_string()).to_string();
        assert!(msg.contains("decode error"), "unexpected: {msg}");
    }

    #[test]
    fn display_missing_extension() {
        let msg = RequestExtractError::MissingExtension {
            type_name: "MySession",
        }
        .to_string();
        assert!(msg.contains("MySession"), "unexpected: {msg}");
    }

    #[test]
    fn request_extract_error_is_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(RequestExtractError::MissingBody);
        assert!(!err.to_string().is_empty());
    }

    // ── header_typed ──────────────────────────────────────────────────────────

    #[test]
    fn header_typed_returns_missing_header_when_absent() {
        let req = make_req(Method::GET, "/", Body::empty());
        let result = req.header_typed::<u32>("x-custom");
        assert!(matches!(
            result,
            Err(RequestExtractError::MissingHeader { .. })
        ));
    }

    #[test]
    fn header_typed_returns_invalid_header_when_not_parseable() {
        let mut req = make_req(Method::GET, "/", Body::empty());
        req.set_header("x-count", "not-a-number");
        let result = req.header_typed::<u64>("x-count");
        assert!(matches!(
            result,
            Err(RequestExtractError::InvalidHeader { .. })
        ));
    }

    #[test]
    fn header_typed_returns_value_when_parseable() {
        let mut req = make_req(Method::GET, "/", Body::empty());
        req.set_header("x-count", "42");
        let result = req.header_typed::<u64>("x-count");
        assert_eq!(result.unwrap(), 42);
    }

    // ── path_param_typed ──────────────────────────────────────────────────────

    #[test]
    fn path_param_typed_returns_missing_when_no_path_params() {
        let req = make_req(Method::GET, "/users/42", Body::empty());
        let result = req.path_param_typed::<u32>("id");
        assert!(matches!(
            result,
            Err(RequestExtractError::MissingPathParameter { .. })
        ));
    }

    #[test]
    fn path_param_typed_returns_invalid_when_wrong_type() {
        use nivasa_routing::RoutePattern;
        let captures = RoutePattern::parse("/users/:id")
            .unwrap()
            .captures("/users/not-a-number")
            .unwrap();
        let mut req = make_req(Method::GET, "/users/not-a-number", Body::empty());
        req.set_path_params(captures);
        let result = req.path_param_typed::<u32>("id");
        assert!(matches!(
            result,
            Err(RequestExtractError::InvalidPathParameter { .. })
        ));
    }

    #[test]
    fn path_param_typed_returns_value_on_success() {
        use nivasa_routing::RoutePattern;
        let captures = RoutePattern::parse("/users/:id")
            .unwrap()
            .captures("/users/99")
            .unwrap();
        let mut req = make_req(Method::GET, "/users/99", Body::empty());
        req.set_path_params(captures);
        let result = req.path_param_typed::<u32>("id");
        assert_eq!(result.unwrap(), 99);
    }

    // ── FromRequest impls ─────────────────────────────────────────────────────

    #[test]
    fn from_request_header_map_clones_headers() {
        let mut req = make_req(Method::GET, "/", Body::empty());
        req.set_header("x-foo", "bar");
        let map = HeaderMap::from_request(&req).unwrap();
        assert!(map.get("x-foo").is_some());
    }

    #[test]
    fn from_request_route_path_captures_missing_returns_error() {
        let req = make_req(Method::GET, "/", Body::empty());
        let result = RoutePathCaptures::from_request(&req);
        assert!(matches!(result, Err(RequestExtractError::MissingPathParameters)));
    }
    #[test]
    fn from_request_body_clones_body() {
        let req = make_req(Method::POST, "/", Body::text("hello"));
        let body = Body::from_request(&req).unwrap();
        assert_eq!(body.as_bytes(), b"hello");
    }

    #[test]
    fn from_request_string_returns_text_body() {
        let req = make_req(Method::POST, "/", Body::text("world"));
        let s = String::from_request(&req).unwrap();
        assert_eq!(s, "world");
    }

    #[test]
    fn from_request_string_empty_body_returns_empty_string() {
        let req = make_req(Method::GET, "/", Body::empty());
        let s = String::from_request(&req).unwrap();
        assert!(s.is_empty());
    }

    #[test]
    fn from_request_json_value_parses_json_body() {
        let req = make_req(Method::POST, "/", Body::json(serde_json::json!({"k": 1})));
        let val = serde_json::Value::from_request(&req).unwrap();
        assert_eq!(val["k"], 1);
    }

    #[test]
    fn from_request_json_value_returns_missing_body_for_empty() {
        let req = make_req(Method::GET, "/", Body::empty());
        let result = serde_json::Value::from_request(&req);
        assert!(matches!(result, Err(RequestExtractError::MissingBody)));
    }

    #[test]
    fn from_request_typed_json_succeeds() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Payload {
            name: String,
        }
        let req = make_req(
            Method::POST,
            "/",
            Body::json(serde_json::json!({"name": "alice"})),
        );
        let Json(payload) = Json::<Payload>::from_request(&req).unwrap();
        assert_eq!(payload.name, "alice");
    }

    #[test]
    fn from_request_typed_json_returns_missing_body_for_empty() {
        #[derive(Debug, Deserialize)]
        struct Payload {
            name: String,
        }
        let req = make_req(Method::POST, "/", Body::empty());
        let result = Json::<Payload>::from_request(&req);
        assert!(matches!(result, Err(RequestExtractError::MissingBody)));
    }

    // ── query helpers ─────────────────────────────────────────────────────────

    #[test]
    fn query_typed_returns_missing_when_absent() {
        let req = make_req(Method::GET, "/users", Body::empty());
        let result = req.query_typed::<u32>("page");
        assert!(matches!(
            result,
            Err(RequestExtractError::MissingQueryParameter { .. })
        ));
    }

    #[test]
    fn query_typed_returns_value_when_present() {
        let req = make_req(Method::GET, "/users?page=3", Body::empty());
        let page: u32 = req.query_typed("page").unwrap();
        assert_eq!(page, 3);
    }

    #[test]
    fn query_typed_returns_invalid_when_wrong_type() {
        let req = make_req(Method::GET, "/users?page=notanumber", Body::empty());
        let result = req.query_typed::<u32>("page");
        assert!(matches!(
            result,
            Err(RequestExtractError::InvalidQueryParameter { .. })
        ));
    }

    // ── NivasaRequest basics ──────────────────────────────────────────────────

    #[test]
    fn new_request_path_and_method() {
        let req = NivasaRequest::new(Method::DELETE, "/users/5", Body::empty());
        assert_eq!(req.method(), &Method::DELETE);
        assert_eq!(req.path(), "/users/5");
    }

    #[test]
    fn set_and_clear_path_params() {
        use nivasa_routing::RoutePattern;
        let captures = RoutePattern::parse("/users/:id")
            .unwrap()
            .captures("/users/42")
            .unwrap();

        let mut req = NivasaRequest::new(Method::GET, "/", Body::empty());
        req.set_path_params(captures);
        assert!(req.path_params().is_some());
        req.clear_path_params();
        assert!(req.path_params().is_none());
    }
}
