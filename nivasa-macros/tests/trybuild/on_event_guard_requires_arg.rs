use nivasa_macros::on_event;

struct EventGateway;

impl EventGateway {
    #[nivasa_macros::guard()]
    #[on_event("user.created")]
    fn on_user_created(&self) {}
}

fn main() {}
