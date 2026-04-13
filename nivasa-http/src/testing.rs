use crate::{Body, NivasaRequest, NivasaServer};
use bytes::Bytes;
use http::{
    header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE},
    Method, Request, Response, StatusCode,
};
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;

/// In-memory HTTP client for dispatching requests through a built server.
pub struct TestClient {
    server: NivasaServer,
    method: Method,
    path: String,
    headers: HeaderMap,
    body: Body,
}

impl TestClient {
    /// Create a test client from a built server.
    pub fn new(server: NivasaServer) -> Self {
        Self {
            server,
            method: Method::GET,
            path: "/".to_string(),
            headers: HeaderMap::new(),
            body: Body::empty(),
        }
    }

    /// Prepare a GET request.
    pub fn get(mut self, path: impl Into<String>) -> Self {
        self.method = Method::GET;
        self.path = path.into();
        self
    }

    /// Prepare a POST request.
    pub fn post(mut self, path: impl Into<String>) -> Self {
        self.method = Method::POST;
        self.path = path.into();
        self
    }

    /// Add or replace a request header.
    pub fn header(mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        let name = HeaderName::from_bytes(name.as_ref().as_bytes())
            .expect("request header name must be valid");
        let value =
            HeaderValue::from_str(value.as_ref()).expect("request header value must be valid");
        self.headers.insert(name, value);
        self
    }

    /// Set the request body.
    pub fn body(mut self, body: impl Into<Body>) -> Self {
        self.body = body.into();
        self
    }

    /// Send the request through the server's in-memory dispatch path.
    pub async fn send(self) -> TestResponse {
        let Self {
            server,
            method,
            path,
            headers,
            body,
        } = self;
        let request = build_request(method, path, headers, body);
        let response = server.dispatch_for_test(request).await;
        TestResponse::from_response(response).await
    }

    /// Send the request through the in-memory dispatch path on a private Tokio runtime.
    pub fn send_blocking(self) -> TestResponse {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime must build")
            .block_on(self.send())
    }
}

fn build_request(
    method: Method,
    path: String,
    headers: HeaderMap,
    body: Body,
) -> NivasaRequest {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .body(body)
        .expect("test request must build");

    for (name, value) in headers.iter() {
        request.headers_mut().insert(name, value.clone());
    }

    if request.headers().get(CONTENT_TYPE).is_none() {
        let content_type = match request.body() {
            Body::Empty => None,
            Body::Text(_) => Some("text/plain; charset=utf-8"),
            Body::Html(_) => Some("text/html; charset=utf-8"),
            Body::Json(_) => Some("application/json"),
            Body::Bytes(_) => Some("application/octet-stream"),
        };

        if let Some(content_type) = content_type {
            request
                .headers_mut()
                .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
        }
    }

    NivasaRequest::from_http(request)
}

/// In-memory test response helper.
#[derive(Debug, Clone)]
pub struct TestResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Bytes,
}

impl TestResponse {
    async fn from_response(response: Response<http_body_util::Full<Bytes>>) -> Self {
        let (parts, body) = response.into_parts();
        let body = body
            .collect()
            .await
            .expect("test response body must collect")
            .to_bytes();

        Self {
            status: parts.status,
            headers: parts.headers,
            body,
        }
    }

    /// Response status as a numeric code.
    pub fn status(&self) -> u16 {
        self.status.as_u16()
    }

    /// Look up a response header by name.
    pub fn header(&self, name: impl AsRef<str>) -> Option<String> {
        HeaderName::from_bytes(name.as_ref().as_bytes())
            .ok()
            .and_then(|name| self.headers.get(name))
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    }

    /// Decode the response body as JSON.
    pub fn json<T>(&self) -> T
    where
        T: DeserializeOwned,
    {
        serde_json::from_slice(&self.body).expect("test response body must be valid JSON")
    }

    /// Decode the response body as UTF-8 text.
    pub fn text(&self) -> String {
        String::from_utf8(self.body.to_vec()).expect("test response body must be valid UTF-8")
    }
}
