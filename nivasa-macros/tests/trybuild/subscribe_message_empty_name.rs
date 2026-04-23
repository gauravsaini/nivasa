use nivasa_macros::subscribe_message;

struct ChatGateway;

impl ChatGateway {
    #[subscribe_message("")]
    fn on_join(&self) {}
}

fn main() {}
