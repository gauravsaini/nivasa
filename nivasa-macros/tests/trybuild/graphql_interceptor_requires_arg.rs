use nivasa_macros::{interceptor, resolver};

struct GraphqlGateway;

impl GraphqlGateway {
    #[interceptor()]
    #[resolver("users")]
    fn users(&self) {}
}

fn main() {}
