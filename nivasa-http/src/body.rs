use bytes::Bytes;

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
    pub(crate) fn content_type(&self) -> Option<&'static str> {
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

    /// Borrow the body as owned bytes.
    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            Body::Empty => Vec::new(),
            Body::Text(text) => text.as_bytes().to_vec(),
            Body::Html(html) => html.as_bytes().to_vec(),
            Body::Json(value) => serde_json::to_vec(value).unwrap_or_default(),
            Body::Bytes(bytes) => bytes.clone(),
        }
    }

    /// Borrow the body as shared bytes for transport edges.
    pub fn as_shared_bytes(&self) -> Bytes {
        match self {
            Body::Empty => Bytes::new(),
            Body::Text(text) => Bytes::copy_from_slice(text.as_bytes()),
            Body::Html(html) => Bytes::copy_from_slice(html.as_bytes()),
            Body::Json(value) => Bytes::from(serde_json::to_vec(value).unwrap_or_default()),
            Body::Bytes(bytes) => Bytes::copy_from_slice(bytes),
        }
    }

    /// Consume the body and return owned bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        match self {
            Body::Empty => Vec::new(),
            Body::Text(text) => text.into_bytes(),
            Body::Html(html) => html.into_bytes(),
            Body::Json(value) => serde_json::to_vec(&value).unwrap_or_default(),
            Body::Bytes(bytes) => bytes,
        }
    }

    /// Consume the body and return shared bytes for transport edges.
    pub fn into_shared_bytes(self) -> Bytes {
        match self {
            Body::Empty => Bytes::new(),
            Body::Text(text) => Bytes::from(text),
            Body::Html(html) => Bytes::from(html),
            Body::Json(value) => Bytes::from(serde_json::to_vec(&value).unwrap_or_default()),
            Body::Bytes(bytes) => Bytes::from(bytes),
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

impl From<Bytes> for Body {
    fn from(value: Bytes) -> Self {
        Self::bytes(value.to_vec())
    }
}

impl From<serde_json::Value> for Body {
    fn from(value: serde_json::Value) -> Self {
        Self::json(value)
    }
}

#[cfg(test)]
mod tests {
    use super::Body;

    #[test]
    fn content_type_matches_body_variant() {
        let cases = [
            (Body::empty(), None),
            (Body::text("plain"), Some("text/plain; charset=utf-8")),
            (Body::html("<p>html</p>"), Some("text/html; charset=utf-8")),
            (
                Body::json(serde_json::json!({ "ok": true })),
                Some("application/json"),
            ),
            (Body::bytes([1_u8, 2, 3]), Some("application/octet-stream")),
        ];

        for (body, expected) in cases {
            assert_eq!(body.content_type(), expected);
        }
    }
}
