use std::time::Duration;

use nivasa_core::module::ConfigurableModule;
use nivasa_http::testing::TestClient;
use nivasa_http::{
    InMemoryThrottlerStorage, NivasaResponse, NivasaServer, RouteThrottleRegistration,
    ThrottlerModule, ThrottlerOptions, ThrottlerOptionsProvider,
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

#[test]
fn throttler_module_for_feature_stays_local() {
    let module = ThrottlerModule::for_feature(ThrottlerOptions::new(5, Duration::from_secs(30)));

    assert!(!module.metadata.is_global);
    assert!(module
        .providers
        .contains(&std::any::TypeId::of::<nivasa_http::ThrottlerGuard>()));
    assert!(module
        .metadata
        .exports
        .contains(&std::any::TypeId::of::<nivasa_http::InMemoryThrottlerStorage>()));
}

#[test]
fn throttler_options_defaults_and_marker_module_are_stable() {
    let options = ThrottlerOptions::default();
    let module = ThrottlerModule::new();

    assert_eq!(options.limit, 10);
    assert_eq!(options.ttl, Duration::from_secs(60));
    assert!(!options.is_global);
    assert_eq!(module, ThrottlerModule);
    assert_eq!(
        ThrottlerOptionsProvider,
        ThrottlerOptionsProvider::default()
    );
}

#[test]
fn throttler_module_root_and_feature_share_provider_contract() {
    let root = <ThrottlerModule as ConfigurableModule>::for_root(
        ThrottlerOptions::default().with_global(true),
    );
    let feature = <ThrottlerModule as ConfigurableModule>::for_feature(ThrottlerOptions::default());
    let storage = std::any::TypeId::of::<nivasa_http::InMemoryThrottlerStorage>();
    let guard = std::any::TypeId::of::<nivasa_http::ThrottlerGuard>();
    let options = std::any::TypeId::of::<ThrottlerOptionsProvider>();

    assert!(root.metadata.is_global);
    assert!(!feature.metadata.is_global);
    for module in [&root, &feature] {
        assert!(module.providers.contains(&storage));
        assert!(module.providers.contains(&guard));
        assert!(module.providers.contains(&options));
        assert!(module.metadata.exports.contains(&storage));
        assert!(module.metadata.exports.contains(&guard));
        assert!(!module.metadata.exports.contains(&options));
    }
}
