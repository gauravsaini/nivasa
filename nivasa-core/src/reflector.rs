//! Read-only metadata access helpers for the shared request context.

use nivasa_common::RequestContext;
use serde::de::DeserializeOwned;

/// A narrow helper for reading metadata from the canonical request context.
///
/// This stays intentionally read-only so later guard and interceptor slices can
/// depend on a stable lookup API without introducing mutation or runtime wiring.
#[derive(Debug, Default, Clone, Copy)]
pub struct Reflector;

impl Reflector {
    /// Create a new reflector helper.
    pub fn new() -> Self {
        Self
    }

    /// Read typed handler metadata from the shared request context.
    pub fn get_handler_metadata<T>(
        &self,
        context: &RequestContext,
        key: &str,
    ) -> Option<T>
    where
        T: DeserializeOwned,
    {
        context
            .handler_metadata(key)
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    /// Read typed class metadata from the shared request context.
    pub fn get_class_metadata<T>(
        &self,
        context: &RequestContext,
        key: &str,
    ) -> Option<T>
    where
        T: DeserializeOwned,
    {
        context
            .class_metadata(key)
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    /// Read typed custom metadata from the shared request context.
    pub fn get_custom_data<T>(
        &self,
        context: &RequestContext,
        key: &str,
    ) -> Option<T>
    where
        T: DeserializeOwned,
    {
        context
            .custom_data(key)
            .cloned()
            .and_then(|value| serde_json::from_value(value).ok())
    }

    /// Read `roles` metadata from the shared request context.
    ///
    /// This prefers handler metadata and falls back to class metadata so
    /// controller-level and route-level role declarations can be consumed with
    /// a single convenience lookup.
    pub fn get_roles(&self, context: &RequestContext) -> Option<Vec<String>> {
        self.get_handler_metadata::<Vec<String>>(context, "roles")
            .or_else(|| self.get_class_metadata::<Vec<String>>(context, "roles"))
    }

    /// Read typed request payload data from the shared request context.
    pub fn get_request_data<'a, T>(&self, context: &'a RequestContext) -> Option<&'a T>
    where
        T: Send + Sync + 'static,
    {
        context.request_data::<T>()
    }
}

#[cfg(test)]
mod tests {
    use super::Reflector;
    use nivasa_common::RequestContext;
    use serde_json::json;

    #[derive(Debug, PartialEq, Eq)]
    struct RequestSnapshot {
        method: &'static str,
        path: &'static str,
    }

    #[test]
    fn reflector_reads_typed_metadata_without_mutating_context() {
        let mut context = RequestContext::new();
        context.set_handler_metadata("roles", json!(["admin", "editor"]));
        context.set_class_metadata("controller", json!("UsersController"));
        context.set_custom_data("request_id", json!("req-123"));
        context.insert_request_data(RequestSnapshot {
            method: "GET",
            path: "/users",
        });

        let reflector = Reflector::new();

        assert_eq!(
            reflector.get_handler_metadata::<Vec<String>>(&context, "roles"),
            Some(vec!["admin".to_string(), "editor".to_string()])
        );
        assert_eq!(
            reflector.get_class_metadata::<String>(&context, "controller"),
            Some("UsersController".to_string())
        );
        assert_eq!(
            reflector.get_custom_data::<String>(&context, "request_id"),
            Some("req-123".to_string())
        );
        assert_eq!(
            reflector.get_roles(&context),
            Some(vec!["admin".to_string(), "editor".to_string()])
        );
        assert_eq!(
            reflector.get_request_data::<RequestSnapshot>(&context),
            Some(&RequestSnapshot {
                method: "GET",
                path: "/users",
            })
        );
    }

    #[test]
    fn reflector_returns_none_for_missing_or_mismatched_metadata() {
        let mut context = RequestContext::new();
        context.set_handler_metadata("enabled", json!(true));

        let reflector = Reflector::new();

        assert_eq!(
            reflector.get_handler_metadata::<String>(&context, "enabled"),
            None
        );
        assert_eq!(
            reflector.get_class_metadata::<String>(&context, "missing"),
            None
        );
        assert_eq!(
            reflector.get_custom_data::<String>(&context, "missing"),
            None
        );
        assert_eq!(reflector.get_roles(&context), None);
        assert_eq!(reflector.get_request_data::<RequestSnapshot>(&context), None);
    }

    #[test]
    fn reflector_falls_back_to_class_roles_when_handler_roles_are_missing() {
        let mut context = RequestContext::new();
        context.set_class_metadata("roles", json!(["reader"]));

        let reflector = Reflector::new();

        assert_eq!(
            reflector.get_roles(&context),
            Some(vec!["reader".to_string()])
        );
    }
}
