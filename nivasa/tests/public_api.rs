use nivasa::prelude::*;

#[test]
fn crate_root_reexports_app_config_types() {
    let server = nivasa::ServerOptions::new("0.0.0.0", 8080)
        .enable_cors()
        .with_global_prefix("api")
        .with_versioning(nivasa::VersioningOptions::new(
            nivasa::VersioningStrategy::Header,
        ).with_default_version(" 1 "));

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
}

#[test]
fn prelude_reexports_app_config_types_for_downstream_use() {
    let server = ServerOptions::default().with_versioning(
        VersioningOptions::new(VersioningStrategy::MediaType).with_default_version("/v2/"),
    );

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

