use nivasa_macros::subscription;

struct GraphqlGateway;

impl GraphqlGateway {
    #[subscription("")]
    fn user_created(&self) {}
}

fn main() {}
