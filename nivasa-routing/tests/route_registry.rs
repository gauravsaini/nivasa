use nivasa_routing::{
    ControllerMetadata, RouteDispatchError, RouteDispatchOutcome, RouteDispatchRegistry,
    RouteMethod, RoutePattern, RouteRegistry, RouteRegistryError,
};

#[test]
fn route_registry_resolves_static_and_pattern_routes() {
    let mut registry = RouteRegistry::new();

    registry.register_static("/health", "health").unwrap();
    registry.register_pattern("/users/:id", "user").unwrap();
    registry.register_pattern("/files/*path", "file").unwrap();

    assert_eq!(registry.resolve("/health"), Some(&"health"));
    assert_eq!(registry.resolve("/users/42"), Some(&"user"));
    assert_eq!(registry.resolve("/files/a/b/c"), Some(&"file"));
    assert_eq!(registry.resolve("/missing"), None);
}

#[test]
fn route_registry_prefers_anchored_routes_and_keeps_fallbacks_available() {
    let mut registry = RouteRegistry::new();

    registry.register_pattern("/:slug?", "fallback").unwrap();
    registry.register_static("/", "root").unwrap();
    registry.register_pattern("/users/:id", "user").unwrap();
    registry.register_pattern("/files/*path", "file").unwrap();

    assert_eq!(registry.resolve("/"), Some(&"root"));
    assert_eq!(registry.resolve("/users/42"), Some(&"user"));
    assert_eq!(registry.resolve("/files/a/b/c"), Some(&"file"));
    assert_eq!(registry.resolve("/other"), Some(&"fallback"));
}

#[test]
fn route_registry_rejects_normalized_duplicates() {
    let mut registry = RouteRegistry::new();

    registry.register_static("/users/", "first").unwrap();

    let err = registry.register_static("users", "second").unwrap_err();
    assert_eq!(
        err,
        RouteRegistryError::DuplicateRoute {
            path: "/users".to_string()
        }
    );
}

#[test]
fn route_dispatch_registry_reports_captures_and_method_not_allowed() {
    let mut registry = RouteDispatchRegistry::new();

    registry.register_static("GET", "/users", "list").unwrap();
    registry
        .register_pattern("POST", "/users/:id", "create")
        .unwrap();

    let matched = registry.resolve_match("POST", "/users/42").unwrap();
    assert_eq!(matched.entry.value, "create");
    assert_eq!(matched.entry.method, RouteMethod::Post);
    assert_eq!(matched.captures.get("id"), Some("42"));

    assert_eq!(
        registry.dispatch("PUT", "/users"),
        RouteDispatchOutcome::MethodNotAllowed {
            path: "/users".to_string(),
            allowed_methods: vec!["GET".to_string()],
        }
    );

    assert_eq!(
        registry.dispatch("GET", "/missing"),
        RouteDispatchOutcome::NotFound
    );
}

#[test]
fn route_dispatch_registry_rejects_duplicate_versioned_routes_and_merges_prefixes() {
    let metadata = ControllerMetadata::new("/users").with_version("1");
    let mut registry = RouteDispatchRegistry::new();

    registry
        .register_versioned_controller_route("GET", &metadata, "/list", "v1-list")
        .unwrap();

    let err = registry
        .register_versioned_controller_route("GET", &metadata, "list/", "second")
        .unwrap_err();
    assert_eq!(
        err,
        RouteDispatchError::DuplicateRoute {
            method: "GET".to_string(),
            path: "/v1/users/list".to_string()
        }
    );

    registry
        .register_controller_route("POST", "/users/", "/create", "create")
        .unwrap();

    assert_eq!(registry.resolve("GET", "/v1/users/list"), Some(&"v1-list"));
    assert_eq!(registry.resolve("POST", "/users/create"), Some(&"create"));
}

#[test]
fn route_pattern_strict_constructor_rejects_optional_segments() {
    let err = RoutePattern::static_path("/users/:id?").unwrap_err();

    assert_eq!(
        err,
        RouteRegistryError::UnsupportedPatternSegment {
            path: "/users/:id?".to_string(),
            segment: ":id?".to_string()
        }
    );
}
