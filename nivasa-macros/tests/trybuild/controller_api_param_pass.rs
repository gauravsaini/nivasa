use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::get("/:id")]
    #[nivasa_macros::api_param(name = "id", description = "User ID")]
    fn show(&self) {}
}

fn main() {
    assert_eq!(
        UsersController::__nivasa_controller_api_param_metadata(),
        vec![("show", vec![("id", "User ID")])],
    );
}
