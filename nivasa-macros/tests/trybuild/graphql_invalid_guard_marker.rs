use nivasa_macros::resolver;

struct GraphqlGateway;

impl GraphqlGateway {
    /// nivasa-guard:
    #[resolver("users")]
    fn users(&self) {}
}

fn main() {}
