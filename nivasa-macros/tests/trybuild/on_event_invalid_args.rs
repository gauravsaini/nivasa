use nivasa_macros::on_event;

struct EventGateway;

impl EventGateway {
    #[on_event(42)]
    fn on_user_created(&self, user_id: String) -> String {
        format!("created:{user_id}")
    }
}

fn main() {}
