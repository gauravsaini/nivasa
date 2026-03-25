use std::sync::Arc;

use nivasa_core::di::Lazy;
use nivasa_macros::injectable;

struct Foo;

#[injectable]
struct Service {
    #[inject]
    foo: Lazy<Arc<Foo>>,
}

fn main() {
    let _ = Service::__NIVASA_INJECTABLE_SCOPE;
}
