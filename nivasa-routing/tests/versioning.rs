use nivasa_routing::{
    parse_api_version_accept, parse_api_version_header, RouteDispatchOutcome, RouteDispatchRegistry,
    RouteMethod,
};

#[test]
fn api_version_parsers_normalize_header_and_accept_inputs() {
    assert_eq!(parse_api_version_header("1"), Some("v1".to_string()));
    assert_eq!(parse_api_version_header(" v2 "), Some("v2".to_string()));
    assert_eq!(
        parse_api_version_accept("application/vnd.app.v1+json"),
        Some("v1".to_string())
    );
    assert_eq!(
        parse_api_version_accept("application/vnd.app.v2+json; charset=utf-8"),
        Some("v2".to_string())
    );
    assert_eq!(
        parse_api_version_accept("application/json, application/vnd.app.v3+json"),
        Some("v3".to_string())
    );
    assert_eq!(parse_api_version_accept("application/json"), None);
}

#[test]
fn versioned_selection_prefers_exact_routes_and_falls_back_cleanly() {
    let mut registry = RouteDispatchRegistry::new();

    registry
        .register_header_versioned_route("GET", "1", "/users", "versioned")
        .unwrap();
    registry
        .register_static("POST", "/users", "fallback")
        .unwrap();

    let exact = registry.select_header_versioned("/users", Some("1"));
    assert_eq!(exact.path(), "/users");
    assert_eq!(exact.version(), Some("v1"));
    assert!(exact.exact_version_match());
    assert_eq!(exact.len(), 1);
    assert_eq!(exact.allowed_methods(), vec!["GET".to_string()]);
    assert_eq!(exact.resolve("GET"), Some(&"versioned"));

    let fallback = registry.select_header_versioned("/users", Some("2"));
    assert_eq!(fallback.version(), Some("v2"));
    assert!(!fallback.exact_version_match());
    assert_eq!(fallback.resolve("POST"), Some(&"fallback"));
    assert_eq!(
        fallback.dispatch("GET"),
        RouteDispatchOutcome::MethodNotAllowed {
            path: "/users".to_string(),
            allowed_methods: vec!["POST".to_string()],
        }
    );
}

#[test]
fn versioned_selection_still_considers_fallback_bucket_routes() {
    let mut registry = RouteDispatchRegistry::new();

    registry
        .register_header_versioned_route("GET", "1", "/users", "versioned")
        .unwrap();
    registry
        .register_pattern("GET", "/:slug?", "fallback")
        .unwrap();

    let exact = registry.select_header_versioned("/users", Some("1"));
    assert_eq!(exact.path(), "/users");
    assert_eq!(exact.version(), Some("v1"));
    assert!(exact.exact_version_match());
    assert_eq!(exact.len(), 1);
    assert_eq!(exact.resolve("GET"), Some(&"versioned"));

    let fallback = registry.select_header_versioned("/users", Some("2"));
    assert_eq!(fallback.path(), "/users");
    assert_eq!(fallback.version(), Some("v2"));
    assert!(!fallback.exact_version_match());
    assert_eq!(fallback.len(), 1);
    assert_eq!(fallback.resolve("GET"), Some(&"fallback"));
}

#[test]
fn versioned_selection_falls_back_to_unversioned_same_path_routes() {
    let mut registry = RouteDispatchRegistry::new();

    registry
        .register_header_versioned_route("GET", "1", "/users", "v1-users")
        .unwrap();
    registry.register_static("GET", "/users", "default").unwrap();
    registry.register_static("POST", "/users", "create").unwrap();

    let fallback = registry.select_header_versioned("/users", Some("2"));
    assert_eq!(fallback.path(), "/users");
    assert_eq!(fallback.version(), Some("v2"));
    assert!(!fallback.exact_version_match());
    assert_eq!(fallback.len(), 2);
    assert_eq!(fallback.allowed_methods(), vec!["GET".to_string(), "POST".to_string()]);
    assert_eq!(fallback.resolve("GET"), Some(&"default"));
    assert_eq!(fallback.resolve("POST"), Some(&"create"));
    assert_eq!(
        fallback.dispatch("DELETE"),
        RouteDispatchOutcome::MethodNotAllowed {
            path: "/users".to_string(),
            allowed_methods: vec!["GET".to_string(), "POST".to_string()],
        }
    );
}

#[test]
fn versioned_dispatch_uses_header_and_accept_parsing() {
    let mut registry = RouteDispatchRegistry::new();

    registry
        .register_header_versioned_route("GET", "1", "/users", "header-v1")
        .unwrap();
    registry
        .register_media_type_versioned_route("GET", "2", "/users", "media-v2")
        .unwrap();
    registry
        .register_static("GET", "/users", "default")
        .unwrap();

    assert_eq!(
        registry.resolve_header_versioned("GET", "/users", Some("1")),
        Some(&"header-v1")
    );
    assert_eq!(
        registry.resolve_media_type_versioned("GET", "/users", Some("application/vnd.app.v2+json")),
        Some(&"media-v2")
    );
    assert_eq!(
        registry.resolve_header_versioned("GET", "/users", Some("3")),
        Some(&"default")
    );
    assert!(matches!(
        registry.dispatch_media_type_versioned(
            "GET",
            "/users",
            Some("application/vnd.app.v2+json")
        ),
        RouteDispatchOutcome::Matched(_)
    ));
}

#[test]
fn versioned_dispatch_falls_back_for_media_type_requests() {
    let mut registry = RouteDispatchRegistry::new();

    registry
        .register_media_type_versioned_route("GET", "2", "/users", "v2-users")
        .unwrap();
    registry
        .register_pattern("GET", "/:slug?", "fallback")
        .unwrap();

    let selection = registry.select_media_type_versioned(
        "/users",
        Some("application/vnd.app.v1+json; charset=utf-8"),
    );
    assert_eq!(selection.path(), "/users");
    assert_eq!(selection.version(), Some("v1"));
    assert!(!selection.exact_version_match());
    assert_eq!(selection.len(), 1);
    assert_eq!(selection.resolve("GET"), Some(&"fallback"));

    assert!(matches!(
        registry.dispatch_media_type_versioned(
            "GET",
            "/users",
            Some("application/vnd.app.v1+json; charset=utf-8"),
        ),
        RouteDispatchOutcome::Matched(entry)
            if entry.method == RouteMethod::Get
                && entry.version.is_none()
                && entry.value == "fallback"
    ));
}
