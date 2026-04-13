//! HTTP exception types for the Nivasa framework.
//!
//! Every exception maps to an HTTP status code and follows the standard
//! error response shape: `{ statusCode, message, error }`.
//!
//! # Example
//!
//! ```rust
//! use nivasa_common::{HttpException, HttpStatus};
//!
//! let err = HttpException::not_found("user not found")
//!     .with_details(serde_json::json!({
//!         "resource": "user",
//!         "id": 42
//!     }));
//!
//! assert_eq!(err.status_code, 404);
//! assert_eq!(err.error, "Not Found");
//! assert_eq!(err.details.unwrap()["resource"], "user");
//!
//! let typed = HttpException::from_status(HttpStatus::BadRequest, "invalid input");
//! assert_eq!(typed.status_code, 400);
//! ```

use serde::Serialize;
use std::{error::Error as StdError, sync::Arc};
use thiserror::Error;

use crate::HttpStatus;

/// Base HTTP exception type.
///
/// This is the common serialized shape used by the framework's HTTP errors.
/// Factory helpers on the type build the standard status-specific variants.
#[derive(Debug, Clone, Serialize, Error)]
#[error("{status_code} {error}: {message}")]
#[serde(rename_all = "camelCase")]
pub struct HttpException {
    pub status_code: u16,
    pub message: String,
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    #[serde(skip)]
    #[source]
    cause: Option<Arc<dyn StdError + Send + Sync + 'static>>,
}

impl HttpException {
    /// Create a new `HttpException` with the given status code and message.
    ///
    /// ```rust
    /// use nivasa_common::HttpException;
    ///
    /// let err = HttpException::new(422, "validation failed");
    /// assert_eq!(err.status_code, 422);
    /// assert_eq!(err.error, "Unprocessable Entity");
    /// ```
    pub fn new(status_code: u16, message: impl Into<String>) -> Self {
        let error = default_error_name(status_code);
        Self {
            status_code,
            message: message.into(),
            error,
            details: None,
            cause: None,
        }
    }

    /// Create a new `HttpException` from a typed HTTP status.
    ///
    /// ```rust
    /// use nivasa_common::{HttpException, HttpStatus};
    ///
    /// let err = HttpException::from_status(HttpStatus::Forbidden, "no access");
    /// assert_eq!(err.status_code, 403);
    /// ```
    pub fn from_status(status: HttpStatus, message: impl Into<String>) -> Self {
        Self::new(status.into(), message)
    }

    /// Attach additional details to the exception payload.
    ///
    /// The details are serialized only when present.
    ///
    /// ```rust
    /// use nivasa_common::HttpException;
    /// use serde_json::json;
    ///
    /// let err = HttpException::bad_request("invalid input")
    ///     .with_details(json!({
    ///         "field": "email",
    ///         "reason": "missing @"
    ///     }));
    ///
    /// assert_eq!(err.status_code, 400);
    /// assert_eq!(err.details.as_ref().unwrap()["field"], "email");
    /// ```
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Attach an underlying cause without changing the serialized payload.
    ///
    /// The cause is used for `std::error::Error::source` only.
    ///
    /// ```rust
    /// use nivasa_common::HttpException;
    /// use std::error::Error;
    ///
    /// let err = HttpException::internal_server_error("boom")
    ///     .with_cause(std::io::Error::other("disk full"));
    ///
    /// assert!(err.source().is_some());
    /// assert_eq!(err.status_code, 500);
    /// ```
    pub fn with_cause(mut self, cause: impl StdError + Send + Sync + 'static) -> Self {
        self.cause = Some(Arc::new(cause));
        self
    }

    // --- Factory methods for common HTTP exceptions ---

    /// Create a `400 Bad Request` exception.
    ///
    /// ```rust
    /// use nivasa_common::HttpException;
    ///
    /// let err = HttpException::bad_request("missing required field");
    /// assert_eq!(err.status_code, 400);
    /// assert_eq!(err.error, "Bad Request");
    /// ```
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400u16, message)
    }

    /// Create a `401 Unauthorized` exception.
    ///
    /// ```rust
    /// use nivasa_common::HttpException;
    ///
    /// let err = HttpException::unauthorized("missing token");
    /// assert_eq!(err.status_code, 401);
    /// assert_eq!(err.error, "Unauthorized");
    /// ```
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(401u16, message)
    }

    /// Create a `402 Payment Required` exception.
    pub fn payment_required(message: impl Into<String>) -> Self {
        Self::new(402u16, message)
    }

    /// Create a `403 Forbidden` exception.
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(403u16, message)
    }

    /// Create a `404 Not Found` exception.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(404u16, message)
    }

    /// Create a `405 Method Not Allowed` exception.
    pub fn method_not_allowed(message: impl Into<String>) -> Self {
        Self::new(405u16, message)
    }

    /// Create a `406 Not Acceptable` exception.
    pub fn not_acceptable(message: impl Into<String>) -> Self {
        Self::new(406u16, message)
    }

    /// Create a `409 Conflict` exception.
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(409u16, message)
    }

    /// Create a `410 Gone` exception.
    pub fn gone(message: impl Into<String>) -> Self {
        Self::new(410u16, message)
    }

    /// Create a `413 Payload Too Large` exception.
    pub fn payload_too_large(message: impl Into<String>) -> Self {
        Self::new(413u16, message)
    }

    /// Create a `415 Unsupported Media Type` exception.
    pub fn unsupported_media_type(message: impl Into<String>) -> Self {
        Self::new(415u16, message)
    }

    /// Create a `422 Unprocessable Entity` exception.
    pub fn unprocessable_entity(message: impl Into<String>) -> Self {
        Self::new(422u16, message)
    }

    /// Create a `429 Too Many Requests` exception.
    pub fn too_many_requests(message: impl Into<String>) -> Self {
        Self::new(429u16, message)
    }

    /// Create a `408 Request Timeout` exception.
    pub fn request_timeout(message: impl Into<String>) -> Self {
        Self::new(408u16, message)
    }

    /// Create a `500 Internal Server Error` exception.
    ///
    /// ```rust
    /// use nivasa_common::HttpException;
    ///
    /// let err = HttpException::internal_server_error("something broke");
    /// assert_eq!(err.status_code, 500);
    /// assert_eq!(err.error, "Internal Server Error");
    /// ```
    pub fn internal_server_error(message: impl Into<String>) -> Self {
        Self::new(500u16, message)
    }

    /// Create a `501 Not Implemented` exception.
    pub fn not_implemented(message: impl Into<String>) -> Self {
        Self::new(501u16, message)
    }

    /// Create a `502 Bad Gateway` exception.
    pub fn bad_gateway(message: impl Into<String>) -> Self {
        Self::new(502u16, message)
    }

    /// Create a `503 Service Unavailable` exception.
    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::new(503u16, message)
    }

    /// Create a `504 Gateway Timeout` exception.
    pub fn gateway_timeout(message: impl Into<String>) -> Self {
        Self::new(504u16, message)
    }
}

fn default_error_name(status_code: u16) -> String {
    HttpStatus::try_from(status_code)
        .map(|status| status.reason_phrase())
        .unwrap_or("Unknown Error")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exception_creation() {
        let ex = HttpException::from_status(HttpStatus::NotFound, "User not found");
        assert_eq!(ex.status_code, 404);
        assert_eq!(ex.message, "User not found");
        assert_eq!(ex.error, "Not Found");
    }

    #[test]
    fn test_exception_status_code_matrix() {
        macro_rules! assert_exception_case {
            ($ctor:expr, $status:expr, $error:expr) => {{
                let ex = ($ctor)("example");
                assert_eq!(ex.status_code, $status);
                assert_eq!(ex.message, "example");
                assert_eq!(ex.error, $error);
            }};
        }

        assert_exception_case!(HttpException::bad_request, 400, "Bad Request");
        assert_exception_case!(HttpException::unauthorized, 401, "Unauthorized");
        assert_exception_case!(HttpException::payment_required, 402, "Payment Required");
        assert_exception_case!(HttpException::forbidden, 403, "Forbidden");
        assert_exception_case!(HttpException::not_found, 404, "Not Found");
        assert_exception_case!(HttpException::method_not_allowed, 405, "Method Not Allowed");
        assert_exception_case!(HttpException::not_acceptable, 406, "Not Acceptable");
        assert_exception_case!(HttpException::request_timeout, 408, "Request Timeout");
        assert_exception_case!(HttpException::conflict, 409, "Conflict");
        assert_exception_case!(HttpException::gone, 410, "Gone");
        assert_exception_case!(HttpException::payload_too_large, 413, "Payload Too Large");
        assert_exception_case!(
            HttpException::unsupported_media_type,
            415,
            "Unsupported Media Type"
        );
        assert_exception_case!(
            HttpException::unprocessable_entity,
            422,
            "Unprocessable Entity"
        );
        assert_exception_case!(HttpException::too_many_requests, 429, "Too Many Requests");
        assert_exception_case!(
            HttpException::internal_server_error,
            500,
            "Internal Server Error"
        );
        assert_exception_case!(HttpException::not_implemented, 501, "Not Implemented");
        assert_exception_case!(HttpException::bad_gateway, 502, "Bad Gateway");
        assert_exception_case!(HttpException::service_unavailable, 503, "Service Unavailable");
        assert_exception_case!(HttpException::gateway_timeout, 504, "Gateway Timeout");
    }

    #[test]
    fn test_exception_serialization() {
        let ex = HttpException::bad_request("Invalid email");
        let json = serde_json::to_value(&ex).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "statusCode": 400,
                "message": "Invalid email",
                "error": "Bad Request"
            })
        );
    }

    #[test]
    fn test_exception_with_details() {
        let ex = HttpException::unprocessable_entity("Validation failed").with_details(
            serde_json::json!({
                "fields": {
                    "email": "must be a valid email"
                }
            }),
        );
        let json = serde_json::to_value(&ex).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "statusCode": 422,
                "message": "Validation failed",
                "error": "Unprocessable Entity",
                "details": {
                    "fields": {
                        "email": "must be a valid email"
                    }
                }
            })
        );
    }

    #[test]
    fn test_exception_display_and_error_traits() {
        let ex = HttpException::internal_server_error("Something broke");

        assert_eq!(ex.to_string(), "500 Internal Server Error: Something broke");

        let err: &dyn std::error::Error = &ex;
        assert!(err.source().is_none());
    }

    #[test]
    fn test_exception_cause_chaining_keeps_serialization_shape() {
        let inner = std::io::Error::other("disk failed");
        let ex = HttpException::internal_server_error("Something broke").with_cause(inner);

        assert_eq!(ex.to_string(), "500 Internal Server Error: Something broke");

        let err: &dyn std::error::Error = &ex;
        assert_eq!(err.source().map(ToString::to_string), Some("disk failed".into()));

        let json = serde_json::to_value(&ex).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "statusCode": 500,
                "message": "Something broke",
                "error": "Internal Server Error"
            })
        );
    }

}
