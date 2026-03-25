use nivasa_macros::injectable;

struct Foo;

#[injectable]
struct BadService {
    #[inject]
    foo: Foo,
}

fn main() {}
