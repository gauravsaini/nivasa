use std::time::Duration;

use nivasa_http::testing::TestClient;
use nivasa_http::{
    InMemoryThrottlerStorage, NivasaResponse, NivasaServer, ThrottlerModule, ThrottlerOptions,
    RouteThrottleRegistration,
};
use nivasa_routing::RouteMethod;

fn build_throttled_server(storage: InMemoryThrottlerStorage) -> NivasaServer {
    NivasaServer::builder()
        .use_throttler_storage(storage)
        .route_with_throttle(
            RouteMethod::Get,
            "/throttle",
            |_| NivasaResponse::text("allowed"),
            RouteThrottleRegistration::new(1, 60),
        )
        .expect("throttled route must register")
        .build()
}

#[tokio::test]
async fn throttled_route_returns_429_on_the_n_plus_one_request() {
    let storage = InMemoryThrottlerStorage::new();

    let first = TestClient::new(build_throttled_server(storage.clone()))
        .get("/throttle")
        .send()
        .await;
    assert_eq!(first.status(), 200);
    assert_eq!(first.text(), "allowed");

    let second = TestClient::new(build_throttled_server(storage))
        .get("/throttle")
        .send()
        .await;
    assert_eq!(second.status(), 429);
    assert_eq!(second.text(), "too many requests");
}

#[test]
fn throttler_module_exposes_storage_and_guard_surface() {
    let module = ThrottlerModule::for_root(
        ThrottlerOptions::new(10, Duration::from_secs(60)).with_global(true),
    );

    assert!(module.metadata.is_global);
    assert!(module
        .providers
        .contains(&std::any::TypeId::of::<nivasa_http::ThrottlerGuard>()));
    assert!(module
        .metadata
        .exports
        .contains(&std::any::TypeId::of::<nivasa_http::InMemoryThrottlerStorage>()));
    assert!(module
        .metadata
        .exports
        .contains(&std::any::TypeId::of::<nivasa_http::ThrottlerGuard>()));
}
