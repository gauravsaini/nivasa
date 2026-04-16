//! # nivasa-common
//!
//! Shared types for the Nivasa framework.
//!
//! This crate re-exports the most common framework helpers from one place:
//! - `RequestContext` for request-scoped metadata and typed request data
//! - `HttpStatus` for typed HTTP status handling
//! - `HttpException` for serializable framework errors
//!
//! Import those types from `nivasa_common` directly when you need to work with
//! request context, HTTP status codes, or framework errors.
//!
//! # Example
//!
//! ```rust
//! use nivasa_common::{HttpException, HttpStatus, RequestContext};
//! use serde_json::json;
//!
//! let mut context = RequestContext::new();
//! context.set_handler_metadata("roles", json!(["admin"]));
//! context.insert_request_data(String::from("req-123"));
//!
//! let err = HttpStatus::BadRequest
//!     .into_exception("invalid payload")
//!     .with_details(json!({ "field": "email" }));
//!
//! assert_eq!(context.handler_metadata("roles").unwrap(), &json!(["admin"]));
//! assert_eq!(context.request_data::<String>().unwrap(), "req-123");
//! assert_eq!(err.status_code, 400);
//! assert_eq!(err.error, "Bad Request");
//! ```

use std::any::{Any, TypeId};
use std::collections::HashMap;

pub mod exceptions;
pub mod http_status;

pub use exceptions::HttpException;
pub use http_status::{HttpStatus, InvalidHttpStatus};

type OpaqueRequestValue = Box<dyn Any + Send + Sync>;

/// Canonical per-request context shared across runtime layers.
///
/// This type stays transport-agnostic on purpose. HTTP-specific layers can
/// insert request-shaped data here, while guards/interceptors can read a shared
/// metadata/custom-data surface without each crate inventing its own context.
///
/// ```rust
/// use nivasa_common::RequestContext;
///
/// #[derive(Debug, PartialEq)]
/// struct TestRequest {
///     method: &'static str,
///     path: &'static str,
/// }
///
/// let mut context = RequestContext::new();
/// context.insert_request_data(TestRequest {
///     method: "GET",
///     path: "/users/42",
/// });
///
/// assert_eq!(
///     context.request_data::<TestRequest>(),
///     Some(&TestRequest {
///         method: "GET",
///         path: "/users/42",
///     })
/// );
/// ```
#[derive(Default)]
pub struct RequestContext {
    request_data: HashMap<TypeId, OpaqueRequestValue>,
    handler_metadata: HashMap<String, serde_json::Value>,
    class_metadata: HashMap<String, serde_json::Value>,
    custom_data: HashMap<String, serde_json::Value>,
}

impl RequestContext {
    /// Create an empty request context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert typed request-scoped data and return the previous value, if any.
    pub fn insert_request_data<T>(&mut self, value: T) -> Option<T>
    where
        T: Send + Sync + 'static,
    {
        self.request_data
            .insert(TypeId::of::<T>(), Box::new(value))
            .and_then(|previous| previous.downcast::<T>().ok().map(|value| *value))
    }

    /// Read typed request-scoped data by concrete type.
    pub fn request_data<T>(&self) -> Option<&T>
    where
        T: Send + Sync + 'static,
    {
        self.request_data
            .get(&TypeId::of::<T>())
            .and_then(|value| value.downcast_ref::<T>())
    }

    /// Store handler-level metadata under a string key.
    pub fn set_handler_metadata(
        &mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Option<serde_json::Value> {
        self.handler_metadata.insert(key.into(), value.into())
    }

    /// Read handler-level metadata by key.
    pub fn handler_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.handler_metadata.get(key)
    }

    /// Store class-level metadata under a string key.
    pub fn set_class_metadata(
        &mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Option<serde_json::Value> {
        self.class_metadata.insert(key.into(), value.into())
    }

    /// Read class-level metadata by key.
    pub fn class_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.class_metadata.get(key)
    }

    /// Store custom runtime data under a string key.
    pub fn set_custom_data(
        &mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Option<serde_json::Value> {
        self.custom_data.insert(key.into(), value.into())
    }

    /// Read custom runtime data by key.
    pub fn custom_data(&self, key: &str) -> Option<&serde_json::Value> {
        self.custom_data.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::RequestContext;
    use serde_json::json;

    #[derive(Debug, PartialEq)]
    struct TestRequest {
        method: &'static str,
        path: &'static str,
    }

    #[test]
    fn request_context_stores_typed_request_data() {
        let mut context = RequestContext::new();

        context.insert_request_data(TestRequest {
            method: "GET",
            path: "/users/42",
        });

        assert_eq!(
            context.request_data::<TestRequest>(),
            Some(&TestRequest {
                method: "GET",
                path: "/users/42",
            })
        );
    }

    #[test]
    fn request_context_tracks_handler_class_and_custom_metadata() {
        let mut context = RequestContext::new();

        context.set_handler_metadata("roles", json!(["admin"]));
        context.set_class_metadata("controller", json!("UsersController"));
        context.set_custom_data("request_id", json!("req-123"));

        assert_eq!(context.handler_metadata("roles"), Some(&json!(["admin"])));
        assert_eq!(
            context.class_metadata("controller"),
            Some(&json!("UsersController"))
        );
        assert_eq!(context.custom_data("request_id"), Some(&json!("req-123")));
    }
}
