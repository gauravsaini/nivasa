use http::StatusCode;
use nivasa_http::{NivasaResponse, TestClient};
use nivasa_routing::RouteMethod;

#[tokio::test]
async fn test_client_dispatches_requests_in_memory() {
    let server = nivasa_http::NivasaServer::builder()
        .route(RouteMethod::Get, "/hello", |request| {
            let mode = request
                .header("x-mode")
                .and_then(|value| value.to_str().ok())
                .unwrap_or("plain");
            NivasaResponse::text(format!("hello:{mode}")).with_header("x-response-kind", "text")
        })
        .expect("route registers")
        .build();

    let client = TestClient::new(server);

    let get = client
        .get("/hello")
        .header("x-mode", "test")
        .header("x-request-id", "req-123")
        .send()
        .await;

    assert_eq!(get.status(), StatusCode::OK.as_u16());
    assert_eq!(get.text(), "hello:test");
    assert_eq!(get.header("x-response-kind"), Some("text".to_string()));
    assert_eq!(get.header("x-request-id"), Some("req-123".to_string()));
}

#[tokio::test]
async fn test_client_supports_body_and_json_accessors() {
    let server = nivasa_http::NivasaServer::builder()
        .route(RouteMethod::Post, "/echo", |request| {
            let body = String::from_utf8(request.body().as_bytes()).expect("utf8 body");
            NivasaResponse::text(body).with_header("x-response-kind", "text")
        })
        .expect("route registers")
        .build();

    let client = TestClient::new(server);

    let post = client.post("/echo").body("posted body").send().await;
    assert_eq!(post.status(), StatusCode::OK.as_u16());
    assert_eq!(post.text(), "posted body");
    assert_eq!(post.header("x-response-kind"), Some("text".to_string()));
}
