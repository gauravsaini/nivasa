use nivasa_macros::resolver;

trait GraphqlGateway {
    #[resolver("users")]
    fn users(&self);
}

fn main() {}
