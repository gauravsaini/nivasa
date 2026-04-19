use nivasa_http::{
    graphql::{EmptyMutation, EmptySubscription, GraphQLSchema},
    Body, GraphQLModule, GraphQLRequest, GraphQLResponse, NivasaServer, TestClient,
};
use serde_json::json;

#[derive(Default)]
struct EchoQuery;

#[async_graphql::Object]
impl EchoQuery {
    async fn echo(&self, value: String) -> String {
        value
    }
}

fn build_server(module: GraphQLModule) -> NivasaServer {
    module
        .register(NivasaServer::builder())
        .expect("graphql routes should register")
        .build()
}

#[test]
fn graphql_post_executes_a_real_schema() {
    let schema = GraphQLSchema::build(EmptyMutation, EmptyMutation, EmptySubscription).finish();
    let server = build_server(GraphQLModule::from_schema(schema).title("Nivasa GraphQL"));

    let response = TestClient::new(server)
        .post("/graphql")
        .body(Body::json(json!({
            "query": "{ __typename }"
        })))
        .send_blocking();

    assert_eq!(response.status(), 200);
    assert_eq!(
        response.header("content-type"),
        Some(String::from("application/json"))
    );

    let value: serde_json::Value = response.json();
    assert_eq!(value["data"]["__typename"], "EmptyMutation");
}

#[test]
fn graphql_from_schema_forwards_variables_and_operation_name() {
    let schema =
        GraphQLSchema::build(EchoQuery::default(), EmptyMutation, EmptySubscription).finish();
    let server = build_server(
        GraphQLModule::from_schema(schema)
            .endpoint_path("/api/graphql")
            .playground_path("/playground"),
    );

    let response = TestClient::new(server)
        .post("/api/graphql")
        .body(Body::json(json!({
            "query": "query Echo($value: String!) { echo(value: $value) }",
            "operationName": "Echo",
            "variables": {
                "value": "bridge"
            }
        })))
        .send_blocking();

    assert_eq!(response.status(), 200);
    assert_eq!(
        response.header("content-type"),
        Some(String::from("application/json"))
    );

    let value: serde_json::Value = response.json();
    assert_eq!(value["data"]["echo"], "bridge");
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

#[test]
fn graphql_request_extensions_reach_executor_and_error_responses_serialize_cleanly() {
    let server = build_server(GraphQLModule::new(
        |request: nivasa_http::GraphQLRequest| {
            assert_eq!(
                request.extensions,
                Some(json!({
                    "trace": true,
                    "depth": 2
                }))
            );
            GraphQLResponse::error("boom")
        },
    ));

    let response = TestClient::new(server)
        .post("/graphql")
        .body(Body::json(json!({
            "query": "{ __typename }",
            "extensions": {
                "trace": true,
                "depth": 2
            }
        })))
        .send_blocking();

    assert_eq!(response.status(), 200);
    assert_eq!(
        response.header("content-type"),
        Some(String::from("application/json"))
    );

    let value: serde_json::Value = response.json();
    assert!(value.get("data").is_none());
    assert_eq!(value["errors"][0]["message"], "boom");
}
