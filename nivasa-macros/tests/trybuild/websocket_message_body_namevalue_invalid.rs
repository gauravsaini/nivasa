use nivasa_macros::{controller, impl_controller};

#[controller("/ws")]
struct ChatGateway;

#[impl_controller]
impl ChatGateway {
    #[nivasa_macros::post("/messages")]
    fn publish(#[nivasa_macros::message_body = "payload"] payload: String) {
        let _ = payload;
    }
}

fn main() {}
