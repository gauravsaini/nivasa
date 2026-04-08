use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
#[nivasa_macros::api_tags("")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::get("/list")]
    fn list(&self) {}
}

fn main() {}
