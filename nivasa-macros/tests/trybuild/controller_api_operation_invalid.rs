use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::get("/list")]
    #[nivasa_macros::api_operation(summary = "")]
    fn list(&self) {}
}

fn main() {}
