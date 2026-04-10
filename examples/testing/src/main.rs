use nivasa_core::DependencyContainer;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
struct GreeterService {
    label: &'static str,
}

impl GreeterService {
    fn greet(&self) -> &'static str {
        self.label
    }
}

#[tokio::main]
async fn main() {
    let container = DependencyContainer::new();
    container
        .register_value(GreeterService { label: "real" })
        .await;

    let real = container.resolve::<GreeterService>().await.expect("real service");
    let test_container = container.create_scope();
    test_container
        .register_value(GreeterService { label: "mock" })
        .await;
    let mock = test_container
        .resolve::<GreeterService>()
        .await
        .expect("mock service");

    print_demo(real, mock);
}

fn print_demo(real: Arc<GreeterService>, mock: Arc<GreeterService>) {
    println!("real={}", real.greet());
    println!("mock={}", mock.greet());
}
