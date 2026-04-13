use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use nivasa_core::{ModuleMetadata, Test};

#[derive(Debug, PartialEq, Eq)]
struct MockConfig {
    label: &'static str,
}

#[derive(Debug)]
struct FactoryBuilt {
    id: usize,
}

#[tokio::test]
async fn testing_module_resolves_value_override() {
    let testing = Test::create_testing_module(
        ModuleMetadata::new().with_providers(vec![std::any::TypeId::of::<MockConfig>()]),
    )
    .override_provider::<MockConfig>()
    .use_value(MockConfig { label: "mock" })
    .compile()
    .await
    .unwrap();

    let config = testing.get::<MockConfig>().await.unwrap();

    assert_eq!(config.label, "mock");
    assert_eq!(
        testing.metadata().providers,
        vec![std::any::TypeId::of::<MockConfig>()]
    );
}

#[tokio::test]
async fn testing_module_resolves_factory_override_as_singleton() {
    let calls = Arc::new(AtomicUsize::new(0));
    let factory_calls = Arc::clone(&calls);

    let testing = Test::create_testing_module(ModuleMetadata::new())
        .override_provider::<FactoryBuilt>()
        .use_factory(move || {
            let id = factory_calls.fetch_add(1, Ordering::SeqCst);
            FactoryBuilt { id }
        })
        .compile()
        .await
        .unwrap();

    let first = testing.get::<FactoryBuilt>().await.unwrap();
    let second = testing.get::<FactoryBuilt>().await.unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(first.id, 0);
    assert!(Arc::ptr_eq(&first, &second));
}

#[tokio::test]
async fn later_override_replaces_earlier_provider_registration() {
    let testing = Test::create_testing_module(ModuleMetadata::new())
        .override_provider::<MockConfig>()
        .use_value(MockConfig { label: "real" })
        .override_provider::<MockConfig>()
        .use_value(MockConfig { label: "mock" })
        .compile()
        .await
        .unwrap();

    let config = testing.get::<MockConfig>().await.unwrap();
    assert_eq!(config.label, "mock");
}
