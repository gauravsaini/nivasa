use nivasa_routing::{RouteDispatchOutcome, RouteDispatchRegistry, RouteMethod};

#[test]
fn header_versioned_dispatch_helpers_normalize_method_input() {
    let mut registry = RouteDispatchRegistry::new();

    registry
        .register_header_versioned_route("GET", "1", "/users/:id", "v1-show")
        .unwrap();

    assert_eq!(
        registry.resolve_header_versioned(" get ", "/users/42", Some("1")),
        Some(&"v1-show")
    );

    let matched = registry
        .resolve_header_match(" get ", "/users/42", Some("1"))
        .unwrap();
    assert_eq!(matched.entry.method, RouteMethod::Get);
    assert_eq!(matched.entry.version(), Some("v1"));
    assert_eq!(matched.entry.value, "v1-show");
    assert_eq!(matched.captures.get("id"), Some("42"));

    assert!(matches!(
        registry.dispatch_header_versioned(" get ", "/users/42", Some("1")),
        RouteDispatchOutcome::Matched(entry)
            if entry.method == RouteMethod::Get
                && entry.version.as_deref() == Some("v1")
                && entry.value == "v1-show"
    ));
}
