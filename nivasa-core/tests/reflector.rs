use nivasa_core::reflector::Reflector;
use nivasa_common::RequestContext;
use serde_json::json;

#[derive(Debug, PartialEq, Eq)]
struct RouteState {
    method: &'static str,
    path: &'static str,
}

#[test]
fn reflector_reads_request_context_metadata() {
    let mut context = RequestContext::new();
    context.set_handler_metadata("roles", json!(["admin", "editor"]));
    context.set_class_metadata("controller", json!("UsersController"));
    context.set_custom_data("request_id", json!("req-123"));
    context.insert_request_data(RouteState {
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
        reflector.get_request_data::<RouteState>(&context),
        Some(&RouteState {
            method: "GET",
            path: "/users",
        })
    );
}

#[test]
fn reflector_returns_none_when_metadata_is_missing_or_typed_incorrectly() {
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
}
