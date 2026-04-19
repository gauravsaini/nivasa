use http::StatusCode;
use nivasa_core::{ModuleMetadata, Test};
use nivasa_http::{Body, NivasaResponse, NivasaServer, TestClient};
use nivasa_routing::RouteMethod;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct CreateWidget {
    name: String,
}

#[tokio::test]
async fn e2e_module_seeded_test_client_flow() {
    let testing = Test::create_testing_module(
        ModuleMetadata::new().with_providers(vec![std::any::TypeId::of::<String>()]),
    )
    .override_provider::<String>()
    .use_value(String::from("module-ready"))
    .compile()
    .await
    .expect("testing module compiles");

    let greeting = testing
        .get::<String>()
        .await
        .expect("string provider resolves");

    let build_server = |greeting: std::sync::Arc<String>| {
        NivasaServer::builder()
            .route(RouteMethod::Get, "/hello", {
                let greeting = greeting.clone();
                move |request| {
                    let mode = request
                        .header("x-mode")
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or("plain");

                    NivasaResponse::text(format!("{}:{mode}", greeting.as_str()))
                        .with_header("x-module-status", greeting.as_str())
                }
            })
            .expect("GET route registers")
            .route(RouteMethod::Post, "/widgets", {
                let greeting = greeting.clone();
                move |request| {
                    let payload = request
                        .extract::<nivasa_http::Json<CreateWidget>>()
                        .expect("request body must deserialize");
                    let mode = request
                        .header("x-mode")
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or("missing");

                    NivasaResponse::new(
                        StatusCode::CREATED,
                        Body::json(serde_json::json!({
                            "name": payload.into_inner().name,
                            "mode": mode,
                            "module": greeting.as_str(),
                        })),
                    )
                    .with_header("x-module-status", greeting.as_str())
                }
            })
            .expect("POST route registers")
            .build()
    };

    let get = TestClient::new(build_server(greeting.clone()))
        .get("/hello")
        .header("x-mode", "route")
        .send()
        .await;
    assert_eq!(get.status(), StatusCode::OK.as_u16());
    assert_eq!(get.text(), "module-ready:route");
    assert_eq!(
        get.header("x-module-status"),
        Some(String::from("module-ready"))
    );

    let post = TestClient::new(build_server(greeting.clone()))
        .post("/widgets")
        .header("x-mode", "create")
        .body(Body::json(serde_json::json!({"name": "Ada"})))
        .send()
        .await;
    assert_eq!(post.status(), StatusCode::CREATED.as_u16());
    assert_eq!(
        post.json::<serde_json::Value>(),
        serde_json::json!({
            "name": "Ada",
            "mode": "create",
            "module": "module-ready",
        })
    );
    assert_eq!(
        post.header("x-module-status"),
        Some(String::from("module-ready"))
    );
}
