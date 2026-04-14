#[path = "../src/graphql_playground.rs"]
mod graphql_playground;

use graphql_playground::graphql_playground_html;

#[test]
fn playground_surface_renders_expected_shell() {
    let html = graphql_playground_html("Nivasa GraphQL", "/graphql");

    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<title>Nivasa GraphQL</title>"));
    assert!(html.contains(r#"<textarea id="query">query Demo { __typename }</textarea>"#));
    assert!(html.contains(r#"<button id="run">Run query</button>"#));
    assert!(html.contains(r#"GraphQL playground for /graphql"#));
}

#[test]
fn playground_surface_escapes_script_and_markup_contexts() {
    let html = graphql_playground_html(
        r#"GraphQL <Playground> & "Safe""#,
        r#"/graphql?topic=demo&mode="play""#,
    );

    assert!(html.contains("&lt;Playground&gt;"));
    assert!(html.contains("&amp;"));
    assert!(html.contains("&quot;Safe&quot;"));
    assert!(html.contains(r#"GraphQL playground for /graphql?topic=demo&amp;mode=&quot;play&quot;"#));
    assert!(html.contains(r#"const endpoint = "/graphql?topic=demo&mode=\"play\"";"#));
}
