use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::get("/:id")]
    #[nivasa_macros::api_param(name = "", description = "User ID")]
    fn show(&self) {}
}

fn main() {}
