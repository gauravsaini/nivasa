use nivasa_macros::{controller, impl_controller};

struct User;

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::get("/:id")]
    #[nivasa_macros::api_response(status = 200, type = User, description = "")]
    fn show(&self) {}
}

fn main() {}
