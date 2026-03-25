use nivasa_macros::injectable;

struct Foo;

#[injectable(scope = "gigantic")]
struct BadScope {
    #[inject]
    foo: std::sync::Arc<Foo>,
}

fn main() {}
