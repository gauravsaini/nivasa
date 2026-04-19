use http::StatusCode;
use nivasa_http::{testing::TestClient, Body, NivasaResponse, NivasaServer};
use nivasa_routing::RouteMethod;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct CreateUser {
    name: String,
}

#[tokio::test]
async fn test_client_get_status_text_and_header_helpers() {
    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/hello", |request| {
            let mut response = NivasaResponse::text("hello from client");

            if let Some(kind) = request
                .header("x-client-kind")
                .and_then(|value| value.to_str().ok())
            {
                response = response.with_header("x-client-kind", kind);
            }

            response.with_header("x-response-kind", "text")
        })
        .expect("route must register")
        .build();

    let response = TestClient::new(server)
        .get("/hello")
        .header("x-client-kind", "get")
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::OK.as_u16());
    assert_eq!(response.text(), "hello from client");
    assert_eq!(response.header("x-response-kind"), Some("text".to_string()));
    assert_eq!(response.header("x-client-kind"), Some("get".to_string()));
}

#[tokio::test]
async fn test_client_post_body_and_json_helpers() {
    let server = NivasaServer::builder()
        .route(RouteMethod::Post, "/users", |request| {
            let payload = request
                .extract::<nivasa_http::Json<CreateUser>>()
                .expect("test request body must deserialize");
            let client_mode = request
                .header("x-client-mode")
                .and_then(|value| value.to_str().ok())
                .unwrap_or("missing");

            NivasaResponse::new(
                StatusCode::CREATED,
                Body::json(serde_json::json!({
                    "name": payload.into_inner().name,
                    "mode": client_mode,
                })),
            )
            .with_header("x-response-kind", "json")
        })
        .expect("route must register")
        .build();

    let response = TestClient::new(server)
        .post("/users")
        .header("x-client-mode", "create")
        .body(Body::json(serde_json::json!({"name": "Ada"})))
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::CREATED.as_u16());
    assert_eq!(response.header("x-response-kind"), Some("json".to_string()));
    assert_eq!(
        response.json::<serde_json::Value>(),
        serde_json::json!({
            "name": "Ada",
            "mode": "create",
        })
    );
}
