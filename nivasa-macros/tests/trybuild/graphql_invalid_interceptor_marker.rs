use nivasa_macros::resolver;

struct GraphqlGateway;

impl GraphqlGateway {
    /// nivasa-interceptor:
    #[resolver("users")]
    fn users(&self) {}
}

fn main() {}
