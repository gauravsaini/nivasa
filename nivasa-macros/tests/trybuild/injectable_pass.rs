use std::sync::Arc;

use nivasa_macros::injectable;

struct Foo;
struct Bar;

#[injectable(scope = "transient")]
struct Service {
    #[inject]
    foo: Arc<Foo>,
    #[inject]
    maybe_bar: Option<Arc<Bar>>,
}

fn main() {
    let _ = Service::__NIVASA_INJECTABLE_SCOPE;
}
