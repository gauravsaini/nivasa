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
fn route_dispatch_registry_maps_helper_errors_and_blank_version_tokens() {
    let mut registry = RouteDispatchRegistry::new();

    let err = registry
        .register_static("GET", "/users/:id", "bad-static")
        .unwrap_err();
    assert_eq!(
        err,
        RouteDispatchError::UnsupportedPatternSegment {
            path: "/users/:id".to_string(),
            segment: ":id".to_string()
        }
    );

    registry
        .register_static("GET", "/users", "default")
        .unwrap();

    let err = registry
        .register_header_versioned_route("GET", "   ", "/users", "versioned")
        .unwrap_err();
    assert_eq!(
        err,
        RouteDispatchError::DuplicateRoute {
            method: "GET".to_string(),
            path: "/users".to_string()
        }
    );
}

#[test]
fn route_errors_and_methods_cover_display_and_other_variants() {
    let registry_error = RouteRegistryError::DuplicateRoute {
        path: "/users".to_string(),
    };
    assert_eq!(registry_error.to_string(), "duplicate route `/users`");
    let registry_error = RouteRegistryError::UnsupportedPatternSegment {
        path: "/users/:id".to_string(),
        segment: ":id".to_string(),
    };
    assert_eq!(
        registry_error.to_string(),
        "unsupported route segment `:id` in `/users/:id`"
    );

    let dispatch_error = RouteDispatchError::DuplicateRoute {
        method: "POST".to_string(),
        path: "/users".to_string(),
    };
    assert_eq!(dispatch_error.to_string(), "duplicate route `POST /users`");
    let dispatch_error = RouteDispatchError::UnsupportedPatternSegment {
        path: "/files/*path/:bad".to_string(),
        segment: ":bad".to_string(),
    };
    assert_eq!(
        dispatch_error.to_string(),
        "unsupported route segment `:bad` in `/files/*path/:bad`"
    );

    assert_eq!(RouteMethod::parse("put").as_str(), "PUT");
    assert_eq!(RouteMethod::parse("delete").as_str(), "DELETE");
    assert_eq!(RouteMethod::parse("patch").as_str(), "PATCH");
    assert_eq!(RouteMethod::parse("head").as_str(), "HEAD");
    assert_eq!(RouteMethod::parse("options").as_str(), "OPTIONS");
    assert_eq!(RouteMethod::parse("trace").as_str(), "TRACE");
    assert_eq!(RouteMethod::from("all").as_str(), "ALL");
    assert_eq!(RouteMethod::from("custom".to_string()).as_str(), "CUSTOM");
}

#[test]
fn versioned_registration_helpers_map_unsupported_pattern_errors() {
    let mut registry = RouteDispatchRegistry::<&'static str>::new();

    let header_error = registry
        .register_header_versioned_route("GET", "1", "/files/*path/:bad", "bad")
        .unwrap_err();
    assert_eq!(
        header_error,
        RouteDispatchError::UnsupportedPatternSegment {
            path: "/files/*path/:bad".to_string(),
            segment: "*path".to_string()
        }
    );

    let media_error = registry
        .register_media_type_versioned_route("GET", "1", "/files/*path/:bad", "bad")
        .unwrap_err();
    assert_eq!(
        media_error,
        RouteDispatchError::UnsupportedPatternSegment {
            path: "/files/*path/:bad".to_string(),
            segment: "*path".to_string()
        }
    );
}

#[test]
fn route_registry_helper_apis_expose_entries_and_contains() {
    let mut registry = RouteRegistry::new();

    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
    assert_eq!(registry.iter().count(), 0);
    assert!(!registry.contains("/users"));
    assert!(registry.resolve_entry("/users").is_none());

    registry.register_static("/users", "users").unwrap();
    registry.register_pattern("/files/*path", "files").unwrap();

    assert!(!registry.is_empty());
    assert_eq!(registry.len(), 2);
    assert_eq!(registry.iter().count(), 2);
    assert!(registry.contains("users/"));
    assert_eq!(
        registry
            .resolve_entry("/users")
            .map(|entry| entry.pattern.path()),
        Some("/users".to_string())
    );
    assert_eq!(
        registry
            .resolve_entry("/files/docs/guide")
            .map(|entry| entry.pattern.path()),
        Some("/files/*path".to_string())
    );
}

#[test]
fn controller_metadata_blank_versions_stay_unversioned() {
    let metadata = ControllerMetadata::new("/users").with_version("   ");

    assert_eq!(metadata.path(), "/users");
    assert_eq!(metadata.version(), None);
    assert_eq!(metadata.versioned_path(), "/users");
    assert_eq!(metadata.versioned_route_path("/list"), "/users/list");
}

#[test]
fn route_dispatch_selection_helper_apis_cover_empty_and_populated_selections() {
    let mut registry = RouteDispatchRegistry::new();

    let empty = registry.select("/missing");
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);
    assert_eq!(empty.iter().count(), 0);
    assert!(empty.resolve_entry("GET").is_none());
    assert!(empty.resolve_match("GET").is_none());
    assert!(empty.allowed_methods().is_empty());

    registry.register_static("GET", "/users", "list").unwrap();
    registry
        .register_static("POST", "/users", "create")
        .unwrap();

    let selection = registry.select("/users/");
    assert!(!selection.is_empty());
    assert_eq!(selection.len(), 2);
    assert_eq!(selection.iter().count(), 2);
    assert_eq!(
        selection.allowed_methods(),
        vec!["GET".to_string(), "POST".to_string()]
    );
    assert_eq!(
        selection.resolve_entry("POST").map(|entry| entry.value),
        Some("create")
    );
    assert_eq!(
        selection
            .resolve_match("POST")
            .map(|matched| matched.entry.value),
        Some("create")
    );
    assert_eq!(selection.resolve("GET"), Some(&"list"));
}

#[test]
fn route_dispatch_registry_helper_apis_cover_collection_and_none_paths() {
    let mut registry = RouteDispatchRegistry::new();

    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
    assert_eq!(registry.iter().count(), 0);
    assert!(registry
        .resolve_versioned("GET", "/users", Some("1"))
        .is_none());
    assert!(registry.resolve_entry("GET", "/users").is_none());

    registry
        .register_header_versioned_route("GET", "1", "/users/:id", "v1-show")
        .unwrap();
    registry
        .register_static("POST", "/users", "create")
        .unwrap();

    assert!(!registry.is_empty());
    assert_eq!(registry.len(), 2);
    assert_eq!(registry.iter().count(), 2);
    assert!(registry
        .resolve_versioned("DELETE", "/users", Some("1"))
        .is_none());
    assert!(registry.resolve_entry("DELETE", "/users").is_none());

    let matched = registry
        .resolve_header_match("GET", "/users/42", Some("1"))
        .unwrap();
    let entry = matched.entry;
    let captures = entry.captures("/users/42").unwrap();
    assert_eq!(captures.get("id"), Some("42"));
    assert!(entry.captures("/users").is_none());
}

#[test]
fn route_dispatch_registry_exposes_entry_contains_and_versioned_match_helpers() {
    let mut registry = RouteDispatchRegistry::new();

    registry
        .register_header_versioned_route("GET", "1", "/users/:id", "v1-show")
        .unwrap();
    registry
        .register_media_type_versioned_route("GET", "2", "/users/:id", "v2-show")
        .unwrap();
    registry
        .register_pattern("GET", "/users/:id", "fallback-show")
        .unwrap();

    assert!(registry.contains("GET", "/users/42"));
    assert!(!registry.contains("DELETE", "/users/42"));
    assert_eq!(
        registry
            .resolve_entry("GET", "/users/42")
            .map(|entry| entry.value),
        Some("fallback-show")
    );

    let header_match = registry
        .resolve_header_match("GET", "/users/42", Some("1"))
        .unwrap();
    assert_eq!(header_match.entry.value, "v1-show");
    assert_eq!(header_match.entry.version(), Some("v1"));
    assert_eq!(header_match.captures.get("id"), Some("42"));

    let media_match = registry
        .resolve_media_type_match(
            "GET",
            "/users/7",
            Some("application/vnd.app.v2+json; charset=utf-8"),
        )
        .unwrap();
    assert_eq!(media_match.entry.value, "v2-show");
    assert_eq!(media_match.entry.version(), Some("v2"));
    assert_eq!(media_match.captures.get("id"), Some("7"));

    let fallback_match = registry
        .resolve_match_versioned("GET", "/users/9", Some("3"))
        .unwrap();
    assert_eq!(fallback_match.entry.value, "fallback-show");
    assert_eq!(fallback_match.entry.version(), None);
    assert_eq!(fallback_match.captures.get("id"), Some("9"));
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

#[test]
fn route_pattern_parse_rejects_invalid_optional_and_empty_parameter_segments() {
    let err = RoutePattern::parse("/users/:id?/posts").unwrap_err();
    assert_eq!(
        err,
        RouteRegistryError::UnsupportedPatternSegment {
            path: "/users/:id?/posts".to_string(),
            segment: ":id?".to_string()
        }
    );

    let err = RoutePattern::parse("/users/name?").unwrap_err();
    assert_eq!(
        err,
        RouteRegistryError::UnsupportedPatternSegment {
            path: "/users/name?".to_string(),
            segment: "name?".to_string()
        }
    );

    let err = RoutePattern::parse("/users/:?").unwrap_err();
    assert_eq!(
        err,
        RouteRegistryError::UnsupportedPatternSegment {
            path: "/users/:?".to_string(),
            segment: ":?".to_string()
        }
    );

    let err = RoutePattern::parse("/users/:").unwrap_err();
    assert_eq!(
        err,
        RouteRegistryError::UnsupportedPatternSegment {
            path: "/users/:".to_string(),
            segment: ":".to_string()
        }
    );
}

#[test]
fn route_dispatch_selection_stays_empty_when_only_other_versions_exist() {
    let mut registry = RouteDispatchRegistry::new();

    registry
        .register_header_versioned_route("GET", "1", "/users", "v1-users")
        .unwrap();

    let selection = registry.select_versioned("/users/", Some("2"));
    assert!(selection.is_empty());
    assert_eq!(selection.path(), "/users");
    assert_eq!(selection.version(), Some("v2"));
    assert!(!selection.exact_version_match());
    assert!(selection.resolve("GET").is_none());
    assert!(selection.resolve_match("GET").is_none());
    assert_eq!(selection.dispatch("GET"), RouteDispatchOutcome::NotFound);
    assert_eq!(
        registry.dispatch_versioned("GET", "/users", Some("2")),
        RouteDispatchOutcome::NotFound
    );
}

#[test]
fn versioned_helper_resolvers_fall_back_when_header_parsers_reject_input() {
    let mut registry = RouteDispatchRegistry::new();

    registry
        .register_static("GET", "/users", "fallback")
        .unwrap();
    registry
        .register_header_versioned_route("GET", "1", "/users", "v1-users")
        .unwrap();
    registry
        .register_media_type_versioned_route("GET", "2", "/users", "v2-users")
        .unwrap();

    assert_eq!(
        registry.resolve_header_versioned("GET", "/users", Some("   ")),
        Some(&"fallback")
    );
    assert_eq!(
        registry.resolve_media_type_versioned("GET", "/users", Some("application/json")),
        Some(&"fallback")
    );
    assert_eq!(
        registry.dispatch_header_versioned("GET", "/users", Some("")),
        RouteDispatchOutcome::Matched(
            registry
                .resolve_entry("GET", "/users")
                .expect("fallback route exists")
        )
    );
}

#[test]
fn route_pattern_captures_expose_unnamed_wildcards_through_iter() {
    let pattern = RoutePattern::parse("/files/*").unwrap();

    let captures = pattern.captures("/files/docs/guide").unwrap();
    let items = captures.iter().collect::<Vec<_>>();

    assert_eq!(captures.len(), 1);
    assert_eq!(captures.get("*"), Some("docs/guide"));
    assert_eq!(captures.wildcard(), Some("docs/guide"));
    assert_eq!(captures.wildcard_name(), None);
    assert_eq!(items, vec![("*", "docs/guide")]);
}
