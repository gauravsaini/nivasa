use nivasa::prelude::*;

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
        .build();
    let bootstrap = nivasa::AppBootstrapConfig::from(server.clone());

    assert_eq!(bootstrap.server, server);
    assert_eq!(
        nivasa::AppBootstrapConfig::default().server,
        ServerOptions::default()
    );
}
