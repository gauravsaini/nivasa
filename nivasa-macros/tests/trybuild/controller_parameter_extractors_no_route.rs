use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    fn create(#[nivasa_macros::header("x-request-id")] request_id: String) {
        let _ = request_id;
    }
}

fn main() {}
