use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use nivasa_core::{ModuleMetadata, Test};

#[derive(Debug, PartialEq, Eq)]
struct RealService {
    label: &'static str,
}

#[tokio::test]
async fn override_replaces_real_provider_with_mock() {
    let real_calls = Arc::new(AtomicUsize::new(0));
    let real_calls_for_factory = Arc::clone(&real_calls);

    let testing = Test::create_testing_module(ModuleMetadata::new())
        .override_provider::<RealService>()
        .use_factory(move || {
            real_calls_for_factory.fetch_add(1, Ordering::SeqCst);
            RealService { label: "real" }
        })
        .override_provider::<RealService>()
        .use_value(RealService { label: "mock" })
        .compile()
        .await
        .unwrap();

    let service = testing.get::<RealService>().await.unwrap();

    assert_eq!(service.label, "mock");
    assert_eq!(real_calls.load(Ordering::SeqCst), 0);
}
