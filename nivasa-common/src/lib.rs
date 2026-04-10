//! # nivasa-common
//!
//! Shared types for the Nivasa framework.
//!
//! This crate provides the foundational types used across all Nivasa crates:
//! - `HttpException` and all standard HTTP exception types (400, 401, 403, 404, 500, etc.)
//! - Common result types and error handling utilities
//! - DTO marker traits
//!
//! # Example
//!
//! ```rust
//! use nivasa_common::RequestContext;
//!
//! let mut context = RequestContext::new();
//! context.set_handler_metadata("roles", serde_json::json!(["admin"]));
//! context.set_custom_data("request_id", serde_json::json!("req-123"));
//!
//! assert_eq!(context.handler_metadata("roles").unwrap(), &serde_json::json!(["admin"]));
//! assert_eq!(context.custom_data("request_id").unwrap(), &serde_json::json!("req-123"));
//! ```

use std::any::{Any, TypeId};
use std::collections::HashMap;

pub mod exceptions;
pub mod http_status;

pub use exceptions::HttpException;
pub use http_status::HttpStatus;

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
