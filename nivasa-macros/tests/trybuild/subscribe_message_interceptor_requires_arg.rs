use nivasa_macros::{interceptor, subscribe_message};

struct ChatGateway;

impl ChatGateway {
    #[interceptor()]
    #[subscribe_message("chat.join")]
    fn on_join(&self) {}
}

fn main() {}
