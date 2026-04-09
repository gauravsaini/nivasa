use nivasa::openapi::{swagger_ui_assets, swagger_ui_index_html, SwaggerUiAsset};

#[test]
fn swagger_ui_assets_emit_a_deterministic_html_shell() {
    let assets = swagger_ui_assets("/api/docs/openapi.json");

    assert_eq!(
        assets,
        vec![SwaggerUiAsset {
            path: "/index.html",
            content_type: "text/html; charset=utf-8",
            body: swagger_ui_index_html("/api/docs/openapi.json"),
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
