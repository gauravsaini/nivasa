use nivasa_routing::{RouteDispatchOutcome, RouteDispatchRegistry, RouteMethod};

#[test]
fn header_versioned_dispatch_falls_back_to_unversioned_all_route() {
    let mut registry = RouteDispatchRegistry::new();

    registry
        .register_header_versioned_route("GET", "1", "/users/:id", "v1-show")
        .unwrap();
    registry
        .register_pattern("ALL", "/users/:id", "fallback-all")
        .unwrap();

    let selection = registry.select_header_versioned("/users/42", Some("2"));
    assert_eq!(selection.version(), Some("v2"));
    assert!(!selection.exact_version_match());
    assert_eq!(selection.allowed_methods(), vec!["ALL".to_string()]);

    let matched = selection.resolve_match("PATCH").unwrap();
    assert_eq!(matched.entry.method, RouteMethod::All);
    assert_eq!(matched.entry.version(), None);
    assert_eq!(matched.entry.value, "fallback-all");
    assert_eq!(matched.captures.get("id"), Some("42"));

    assert!(matches!(
        registry.dispatch_header_versioned(" patch ", "/users/42", Some("2")),
        RouteDispatchOutcome::Matched(entry)
            if entry.method == RouteMethod::All
                && entry.version.is_none()
                && entry.value == "fallback-all"
    ));
}
