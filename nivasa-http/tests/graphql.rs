use nivasa_http::{Body, GraphQLModule, GraphQLRequest, GraphQLResponse, NivasaServer, TestClient};
use serde_json::json;

fn build_server(module: GraphQLModule) -> NivasaServer {
    module
        .register(NivasaServer::builder())
        .expect("graphql routes should register")
        .build()
}

#[test]
fn graphql_post_executes_via_the_provided_handler() {
    let server = build_server(
        GraphQLModule::new(|request: GraphQLRequest| {
            assert_eq!(
                request.query,
                "query Hello($name: String!) { hello(name: $name) }"
            );
            assert_eq!(request.operation_name.as_deref(), Some("Hello"));
            assert_eq!(
                request
                    .variables
                    .as_ref()
                    .and_then(|value| value["name"].as_str()),
                Some("Nivasa")
            );

            GraphQLResponse::data(json!({
                "hello": "hi Nivasa",
                "operation": request.operation_name,
            }))
        })
        .title("Nivasa GraphQL"),
    );

    let response = TestClient::new(server)
        .post("/graphql")
        .body(Body::json(json!({
            "query": "query Hello($name: String!) { hello(name: $name) }",
            "operationName": "Hello",
            "variables": { "name": "Nivasa" }
        })))
        .send_blocking();

    assert_eq!(response.status(), 200);
    assert_eq!(
        response.header("content-type"),
        Some(String::from("application/json"))
    );

    let value: serde_json::Value = response.json();
    assert_eq!(value["data"]["hello"], "hi Nivasa");
    assert_eq!(value["data"]["operation"], "Hello");
}

#[test]
fn graphql_get_serves_the_playground_ui() {
    let server = build_server(
        GraphQLModule::new(|request: GraphQLRequest| {
            GraphQLResponse::data(json!({ "query": request.query }))
        })
        .endpoint_path("/graphql")
        .playground_path("/graphql"),
    );

    let response = TestClient::new(server).get("/graphql").send_blocking();

    assert_eq!(response.status(), 200);
    assert_eq!(
        response.header("content-type"),
        Some(String::from("text/html; charset=utf-8"))
    );

    let html = response.text();
    assert!(html.contains("Nivasa GraphQL"));
    assert!(html.contains("GraphQL playground for /graphql"));
    assert!(html.contains("fetch(endpoint"));
}

#[test]
fn graphql_invalid_body_returns_a_bad_request_response() {
    let server = build_server(GraphQLModule::new(|_| {
        GraphQLResponse::data(json!({ "ok": true }))
    }));

    let response = TestClient::new(server)
        .post("/graphql")
        .body(Body::json(json!({ "operationName": "Demo" })))
        .send_blocking();

    assert_eq!(response.status(), 400);
    assert_eq!(
        response.header("content-type"),
        Some(String::from("application/json"))
    );

    let value: serde_json::Value = response.json();
    assert!(value["errors"][0]["message"]
        .as_str()
        .expect("error message")
        .contains("missing field"));
}
