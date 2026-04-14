use nivasa_macros::mutation;

struct GraphqlGateway;

impl GraphqlGateway {
    #[mutation(42)]
    fn create_user(&self) {}
}

fn main() {}
