use nivasa_macros::subscribe_message;

struct ChatGateway;

impl ChatGateway {
    #[nivasa_macros::guard()]
    #[subscribe_message("chat.join")]
    fn on_join(&self) {}
}

fn main() {}
