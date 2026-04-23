use http::StatusCode;
use nivasa_http::{NivasaResponse, NivasaServer, TestClient};
use nivasa_routing::RouteMethod;
use uuid::Uuid;

#[tokio::test]
async fn test_client_generates_request_id_for_not_found_dispatch_responses() {
    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/known", |_| {
            NivasaResponse::text("known")
        })
        .expect("route must register")
        .build();

    let response = TestClient::new(server).get("/missing").send().await;

    let request_id = response
        .header("x-request-id")
        .expect("dispatch response must include a generated request id");

    assert_eq!(response.status(), StatusCode::NOT_FOUND.as_u16());
    assert_eq!(response.text(), "not found");
    Uuid::parse_str(&request_id).expect("generated request id should be a UUID");
}

#[tokio::test]
async fn test_client_preserves_request_id_and_allow_header_for_method_not_allowed() {
    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/known", |_| {
            NivasaResponse::text("known")
        })
        .expect("route must register")
        .build();

    let response = TestClient::new(server)
        .post("/known")
        .header("x-request-id", "req-405")
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED.as_u16());
    assert_eq!(response.text(), "method not allowed");
    assert_eq!(response.header("allow"), Some(String::from("GET")));
    assert_eq!(
        response.header("x-request-id"),
        Some(String::from("req-405"))
    );
}

#[tokio::test]
async fn test_client_generates_request_id_for_blank_method_not_allowed_requests() {
    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/known", |_| NivasaResponse::text("get"))
        .expect("get route must register")
        .build();

    let response = TestClient::new(server)
        .post("/known")
        .header("x-request-id", "   ")
        .send()
        .await;

    let request_id = response
        .header("x-request-id")
        .expect("dispatch response must include a generated request id");

    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED.as_u16());
    assert_eq!(response.text(), "method not allowed");
    assert_eq!(response.header("allow"), Some(String::from("GET")));
    Uuid::parse_str(&request_id).expect("generated request id should be a UUID");
}
