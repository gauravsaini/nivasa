//! HTTP exception types for the Nivasa framework.
//!
//! Every exception maps to an HTTP status code and follows the standard
//! error response shape: `{ statusCode, message, error }`.

use serde::Serialize;
use std::fmt;

use crate::HttpStatus;

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

    /// Create a new HttpException from a typed HTTP status.
    pub fn from_status(status: HttpStatus, message: impl Into<String>) -> Self {
        Self::new(status.into(), message)
    }

    /// Attach additional details to the exception.
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    // --- Factory methods for common HTTP exceptions ---

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400u16, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(401u16, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(403u16, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(404u16, message)
    }

    pub fn method_not_allowed(message: impl Into<String>) -> Self {
        Self::new(405u16, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(409u16, message)
    }

    pub fn unprocessable_entity(message: impl Into<String>) -> Self {
        Self::new(422u16, message)
    }

    pub fn too_many_requests(message: impl Into<String>) -> Self {
        Self::new(429u16, message)
    }

    pub fn internal_server_error(message: impl Into<String>) -> Self {
        Self::new(500u16, message)
    }

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::new(503u16, message)
    }
}

impl fmt::Display for HttpException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}: {}", self.status_code, self.error, self.message)
    }
}

impl std::error::Error for HttpException {}

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
