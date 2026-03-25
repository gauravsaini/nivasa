use std::sync::Arc;
use nivasa_core::di::{DependencyContainer, ProviderScope};
use nivasa_macros::{injectable, module};
use nivasa_core::module::Module;

#[injectable]
struct ServiceA {
    // No dependencies
}

#[injectable]
struct ServiceB {
    #[allow(dead_code)]
    a: Arc<ServiceA>,
}

#[module({
    providers: [ServiceA, ServiceB],
    exports: [ServiceB]
})]
struct RootModule;

#[tokio::test]
async fn test_full_di_lifecycle() {
    let container = DependencyContainer::new();
    let root_module = RootModule;
    
    // Configure the module (registers providers)
    root_module.configure(&container).await.unwrap();
    
    // Initialize the container (validates graph and builds singletons)
    container.initialize().await.unwrap();
    
    // Resolve ServiceB
    let b = container.resolve::<ServiceB>().await.unwrap();
    
    // Verify ServiceB has ServiceA
    assert!(Arc::strong_count(&b.a) >= 1);
}
