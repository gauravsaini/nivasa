use nivasa_macros::{interceptor, on_event};

struct EventGateway;

impl EventGateway {
    #[interceptor()]
    #[on_event("user.created")]
    fn on_user_created(&self) {}
}

fn main() {}
