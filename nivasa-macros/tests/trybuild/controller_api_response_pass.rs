use nivasa_macros::{controller, impl_controller};

struct User;

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::get("/:id")]
    #[nivasa_macros::api_response(status = 200, type = User, description = "Success")]
    fn show(&self) {}
}

fn main() {
    assert_eq!(
        UsersController::__nivasa_controller_api_response_metadata(),
        vec![("show", vec![(200, "User", "Success")])],
    );
}
