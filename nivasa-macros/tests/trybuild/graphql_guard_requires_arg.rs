use nivasa_macros::resolver;

struct GraphqlGateway;

impl GraphqlGateway {
    #[nivasa_macros::guard()]
    #[resolver("users")]
    fn users(&self) {}
}

fn main() {}
