//! HTTP exception types for the Nivasa framework.
//!
//! Every exception maps to an HTTP status code and follows the standard
//! error response shape: `{ statusCode, message, error }`.

use serde::Serialize;
use std::fmt;

/// Base HTTP exception type.
///
/// All specific exception types (BadRequest, NotFound, etc.) are created
/// via constructor functions on this type.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpException {
    pub status_code: u16,
    pub message: String,
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl HttpException {
    /// Create a new HttpException with the given status code and message.
    pub fn new(status_code: u16, message: impl Into<String>) -> Self {
        let error = default_error_name(status_code);
        Self {
            status_code,
            message: message.into(),
            error,
            details: None,
        }
    }

    /// Attach additional details to the exception.
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    // --- Factory methods for common HTTP exceptions ---

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(401, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(403, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(404, message)
    }

    pub fn method_not_allowed(message: impl Into<String>) -> Self {
        Self::new(405, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(409, message)
    }

    pub fn unprocessable_entity(message: impl Into<String>) -> Self {
        Self::new(422, message)
    }

    pub fn too_many_requests(message: impl Into<String>) -> Self {
        Self::new(429, message)
    }

    pub fn internal_server_error(message: impl Into<String>) -> Self {
        Self::new(500, message)
    }

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::new(503, message)
    }
}

impl fmt::Display for HttpException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}: {}", self.status_code, self.error, self.message)
    }
}

impl std::error::Error for HttpException {}

fn default_error_name(status_code: u16) -> String {
    match status_code {
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Unknown Error",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exception_creation() {
        let ex = HttpException::not_found("User not found");
        assert_eq!(ex.status_code, 404);
        assert_eq!(ex.message, "User not found");
        assert_eq!(ex.error, "Not Found");
    }

    #[test]
    fn test_exception_serialization() {
        let ex = HttpException::bad_request("Invalid email");
        let json = serde_json::to_value(&ex).unwrap();
        assert_eq!(json["statusCode"], 400);
        assert_eq!(json["message"], "Invalid email");
        assert_eq!(json["error"], "Bad Request");
        assert!(json.get("details").is_none());
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
        assert_eq!(json["statusCode"], 422);
        assert!(json["details"]["fields"]["email"].is_string());
    }
}
