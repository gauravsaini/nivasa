use nivasa::openapi::swagger_ui_index_html;
use nivasa::AppBootstrapConfig;

#[test]
fn swagger_ui_route_serves_html_with_the_default_mount_path() {
    let bootstrap = AppBootstrapConfig::default();

    assert_eq!(bootstrap.swagger_ui_path(), "/api/docs");
    assert!(bootstrap.serve_swagger_ui().is_ok());

    let html = swagger_ui_index_html(
        "/api/docs/openapi.json",
        "Nivasa API Docs",
        "OpenAPI documentation",
        "1.0.0",
    );

    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<div id=\"swagger-ui\"></div>"));
    assert!(html.contains(r#"url: "/api/docs/openapi.json""#));
}

#[test]
fn swagger_ui_route_honors_custom_mount_path() {
    let bootstrap = AppBootstrapConfig::default().with_swagger_ui_path("docs");

    assert_eq!(bootstrap.swagger_ui_path(), "/docs");
    assert!(bootstrap.serve_swagger_ui().is_ok());
}
