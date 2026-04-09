use nivasa::openapi::{
    swagger_ui_assets, swagger_ui_assets_with_metadata, swagger_ui_index_html, SwaggerUiAsset,
};

#[test]
fn swagger_ui_assets_emit_a_deterministic_html_shell() {
    let assets = swagger_ui_assets("/api/docs/openapi.json");

    assert_eq!(
        assets,
        vec![SwaggerUiAsset {
            path: "/index.html",
            content_type: "text/html; charset=utf-8",
            body: swagger_ui_index_html(
                "/api/docs/openapi.json",
                "Nivasa API Docs",
                "OpenAPI documentation",
                "1.0.0",
            ),
        }]
    );
    assert!(assets[0]
        .body
        .contains("https://cdn.jsdelivr.net/npm/swagger-ui-dist@5.17.14/swagger-ui.css"));
    assert!(assets[0]
        .body
        .contains("https://cdn.jsdelivr.net/npm/swagger-ui-dist@5.17.14/swagger-ui-bundle.js"));
    assert!(assets[0].body.contains(r#"url: "/api/docs/openapi.json""#));
}

#[test]
fn swagger_ui_assets_render_custom_metadata() {
    let assets = swagger_ui_assets_with_metadata(
        "/docs/openapi.json",
        "Acme API",
        "Internal docs for Acme",
        "2.3.4",
    );

    let body = &assets[0].body;
    assert!(body.contains("<title>Acme API v2.3.4</title>"));
    assert!(body.contains(r#"<meta name="description" content="Internal docs for Acme">"#));
    assert!(body.contains("<h1>Acme API</h1>"));
    assert!(body.contains("<p>Internal docs for Acme</p>"));
    assert!(body.contains(r#"<span class="swagger-ui-version">v2.3.4</span>"#));
    assert!(body.contains(r#"url: "/docs/openapi.json""#));
}
