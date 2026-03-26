use nivasa::prelude::*;
#[allow(unused_imports)]
use nivasa::prelude::{injectable, module, scxml_handler};

#[test]
fn crate_root_reexports_app_config_builders() {
    let versioning = nivasa::VersioningOptions::builder(nivasa::VersioningStrategy::Header)
        .default_version(" 1 ")
        .build();
    let server = nivasa::ServerOptions::builder()
        .host("0.0.0.0")
        .port(8080)
        .enable_cors()
        .global_prefix("api")
        .versioning(versioning.clone())
        .build();

    assert_eq!(server.host, "0.0.0.0");
    assert_eq!(server.port, 8080);
    assert!(server.cors);
    assert_eq!(server.global_prefix.as_deref(), Some("/api"));
    assert_eq!(
        server.versioning.as_ref().map(|options| options.strategy),
        Some(nivasa::VersioningStrategy::Header)
    );
    assert_eq!(
        server.versioning.as_ref().and_then(|options| options.default_version.as_deref()),
        Some("v1")
    );
    assert_eq!(versioning.default_version.as_deref(), Some("v1"));
}

#[test]
fn prelude_reexports_app_config_types_for_downstream_use() {
    let server = ServerOptions::builder()
        .versioning(
            VersioningOptions::builder(VersioningStrategy::MediaType)
                .default_version("/v2/")
                .build(),
        )
        .build();

    assert_eq!(server.host, "127.0.0.1");
    assert_eq!(server.port, 3000);
    assert_eq!(
        server.versioning.as_ref().map(|options| options.strategy),
        Some(VersioningStrategy::MediaType)
    );
    assert_eq!(
        server.versioning.as_ref().and_then(|options| options.default_version.as_deref()),
        Some("v2")
    );
}

#[test]
fn builder_defaults_match_the_existing_config_surface() {
    let server = ServerOptions::builder().build();

    assert_eq!(server.host, "127.0.0.1");
    assert_eq!(server.port, 3000);
    assert!(!server.cors);
    assert_eq!(server.global_prefix, None);
    assert_eq!(server.versioning, None);
}

#[test]
fn crate_root_reexports_bootstrap_config_as_pure_data() {
    let server = ServerOptions::builder()
        .host("0.0.0.0")
        .port(8080)
        .versioning(
            VersioningOptions::builder(VersioningStrategy::Uri)
                .default_version(" v1 ")
                .build(),
        )
        .build();
    let bootstrap = nivasa::AppBootstrapConfig::from(server.clone());

    assert_eq!(bootstrap.server, server);
    assert_eq!(
        bootstrap.versioning().map(|options| options.strategy),
        Some(VersioningStrategy::Uri)
    );
    assert_eq!(
        bootstrap
            .versioning()
            .and_then(|options| options.default_version.as_deref()),
        Some("v1")
    );
    assert_eq!(
        nivasa::AppBootstrapConfig::default().server,
        ServerOptions::default()
    );
    assert_eq!(nivasa::AppBootstrapConfig::default().versioning(), None);
}

#[test]
fn prelude_reexports_core_traits_macros_and_http_types() {
    fn _asserts_module_trait_name_is_in_scope<T: Module>() {}
    fn _asserts_injectable_trait_name_is_in_scope<T: Injectable>() {}

    let _container = DependencyContainer::new();
    let _ = ProviderScope::Singleton;
    let _ = HttpStatus::Ok;
    let _ = HttpException::bad_request("boom");
}

#[test]
fn bootstrap_config_exposes_a_normalized_global_prefix_for_route_setup() {
    let bootstrap = nivasa::AppBootstrapConfig::from(
        ServerOptions::builder()
            .global_prefix(" api/ ")
            .build(),
    );

    assert_eq!(bootstrap.global_prefix(), Some("/api"));
}

#[test]
fn bootstrap_config_can_compose_prefixed_route_paths_without_runtime_wiring() {
    let bootstrap = nivasa::AppBootstrapConfig::from(
        ServerOptions::builder()
            .global_prefix("api")
            .build(),
    );

    assert_eq!(bootstrap.prefixed_route_path("users"), "/api/users");
    assert_eq!(bootstrap.prefixed_route_path("/"), "/api");
    assert_eq!(
        nivasa::AppBootstrapConfig::default().prefixed_route_path("users"),
        "/users"
    );
}

#[cfg(feature = "config")]
#[test]
#[allow(unused_imports)]
fn optional_crate_features_reexport_placeholder_crates_when_enabled() {
    use nivasa::config as config_crate;
    use nivasa::validation as validation_crate;
    use nivasa::websocket as websocket_crate;
}
