use std::sync::Arc;

use nivasa_core::di::DependencyContainer;
use nivasa_macros::injectable;

struct Foo;

#[injectable(scope = "scoped")]
struct GenericService<T> {
    #[inject]
    dep: Arc<T>,
}

fn main() {
    let container = DependencyContainer::new();
    let _scope = GenericService::<Foo>::__NIVASA_INJECTABLE_SCOPE;
    let _future = GenericService::<Foo>::__nivasa_register(&container);
}
