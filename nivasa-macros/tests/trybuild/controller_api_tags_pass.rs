use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
#[nivasa_macros::api_tags("Users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::get("/list")]
    fn list(&self) {}
}

fn main() {
    assert_eq!(UsersController::__nivasa_controller_api_tags(), vec!["Users"]);
}
