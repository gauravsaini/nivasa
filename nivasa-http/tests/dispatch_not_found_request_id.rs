use http::StatusCode;
use nivasa_http::{NivasaResponse, NivasaServer, TestClient};
use nivasa_routing::RouteMethod;

#[tokio::test]
async fn test_client_preserves_request_id_for_not_found_dispatch_responses() {
    let server = NivasaServer::builder()
        .route(RouteMethod::Get, "/known", |_| {
            NivasaResponse::text("known")
        })
        .expect("route must register")
        .build();

    let response = TestClient::new(server)
        .get("/missing")
        .header("x-request-id", "req-404")
        .send()
        .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND.as_u16());
    assert_eq!(response.text(), "not found");
    assert_eq!(response.header("allow"), None);
    assert_eq!(
        response.header("x-request-id"),
        Some(String::from("req-404"))
    );
}
