use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use nivasa_core::di::{DependencyContainer, DiError, Lazy, ProviderScope};
use nivasa_core::module::Module;
use nivasa_macros::{injectable, module};

#[derive(Debug)]
#[injectable]
struct ServiceA;

#[injectable]
struct ServiceB {
    pub a: Arc<ServiceA>,
}

#[module({
    providers: [ServiceA, ServiceB],
    exports: [ServiceB]
})]
struct RootModule;

#[derive(Debug)]
struct OptionalLeaf {
    pub label: &'static str,
}

#[injectable]
struct OptionalConsumer {
    pub maybe_leaf: Option<Arc<OptionalLeaf>>,
}

#[derive(Debug)]
struct LazyLeaf {
    pub id: usize,
}

#[injectable]
struct LazyConsumer {
    pub leaf: Lazy<Arc<LazyLeaf>>,
}

#[derive(Debug)]
struct SharedSingleton {
    pub id: usize,
}

#[injectable]
struct DiamondLeft {
    pub shared: Arc<SharedSingleton>,
}

#[injectable]
struct DiamondRight {
    pub shared: Arc<SharedSingleton>,
}

#[injectable]
struct DiamondRoot {
    pub left: Arc<DiamondLeft>,
    pub right: Arc<DiamondRight>,
}

#[tokio::test]
async fn test_full_di_lifecycle() {
    let container = DependencyContainer::new();
    let root_module = RootModule;

    root_module.configure(&container).await.unwrap();
    container.initialize().await.unwrap();

    let b = container.resolve::<ServiceB>().await.unwrap();
    assert!(Arc::strong_count(&b.a) >= 1);
}

#[tokio::test]
async fn test_missing_provider_returns_clear_error() {
    let container = DependencyContainer::new();

    let err = container.resolve::<ServiceA>().await.unwrap_err();
    assert!(
        matches!(err, DiError::ProviderNotFound(type_name) if type_name == std::any::type_name::<ServiceA>())
    );
}

#[tokio::test]
async fn test_optional_dependency_handles_none_and_some() {
    let empty_container = DependencyContainer::new();
    empty_container
        .register_injectable::<OptionalConsumer>(
            ProviderScope::Singleton,
            <OptionalConsumer as nivasa_core::di::provider::Injectable>::dependencies(),
        )
        .await;
    empty_container.initialize().await.unwrap();

    let empty = empty_container.resolve::<OptionalConsumer>().await.unwrap();
    assert!(empty.maybe_leaf.is_none());

    let populated_container = DependencyContainer::new();
    populated_container
        .register_value(OptionalLeaf { label: "present" })
        .await;
    populated_container
        .register_injectable::<OptionalConsumer>(
            ProviderScope::Singleton,
            <OptionalConsumer as nivasa_core::di::provider::Injectable>::dependencies(),
        )
        .await;
    populated_container.initialize().await.unwrap();

    let populated = populated_container
        .resolve::<OptionalConsumer>()
        .await
        .unwrap();
    let leaf = populated
        .maybe_leaf
        .as_ref()
        .expect("optional dependency should be present");
    assert_eq!(leaf.label, "present");
}

#[tokio::test]
async fn test_lazy_dependency_resolves_on_first_access() {
    let container = DependencyContainer::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let calls = counter.clone();

    container
        .register_factory::<LazyLeaf, _>(ProviderScope::Singleton, vec![], move |_| {
            let calls = calls.clone();
            Box::pin(async move {
                let id = calls.fetch_add(1, Ordering::SeqCst);
                Ok(LazyLeaf { id })
            })
        })
        .await;

    container
        .register_injectable::<LazyConsumer>(
            ProviderScope::Singleton,
            <LazyConsumer as nivasa_core::di::provider::Injectable>::dependencies(),
        )
        .await;

    let consumer = container.resolve::<LazyConsumer>().await.unwrap();
    assert_eq!(counter.load(Ordering::SeqCst), 0);

    let first = consumer.leaf.get().await.unwrap();
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    assert_eq!(first.id, 0);

    let second = consumer.leaf.get().await.unwrap();
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    assert!(Arc::ptr_eq(&first, &second));
}

#[tokio::test]
async fn test_register_value_factory_and_diamond_shared_singleton() {
    let container = DependencyContainer::new();
    let counter = Arc::new(AtomicUsize::new(0));
    let calls = counter.clone();

    container.register_value(String::from("config")).await;
    let config = container.resolve::<String>().await.unwrap();
    assert_eq!(config.as_str(), "config");

    container
        .register_factory::<SharedSingleton, _>(ProviderScope::Singleton, vec![], move |_| {
            let calls = calls.clone();
            Box::pin(async move {
                let id = calls.fetch_add(1, Ordering::SeqCst);
                Ok(SharedSingleton { id })
            })
        })
        .await;

    container
        .register_injectable::<DiamondLeft>(
            ProviderScope::Singleton,
            <DiamondLeft as nivasa_core::di::provider::Injectable>::dependencies(),
        )
        .await;
    container
        .register_injectable::<DiamondRight>(
            ProviderScope::Singleton,
            <DiamondRight as nivasa_core::di::provider::Injectable>::dependencies(),
        )
        .await;
    container
        .register_injectable::<DiamondRoot>(
            ProviderScope::Singleton,
            <DiamondRoot as nivasa_core::di::provider::Injectable>::dependencies(),
        )
        .await;
    container.initialize().await.unwrap();

    let root = container.resolve::<DiamondRoot>().await.unwrap();
    assert!(Arc::ptr_eq(&root.left.shared, &root.right.shared));
    assert_eq!(root.left.shared.id, 0);
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}
