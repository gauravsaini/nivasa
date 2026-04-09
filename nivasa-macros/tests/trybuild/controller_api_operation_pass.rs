use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::get("/list")]
    #[nivasa_macros::api_operation(summary = "Get all users")]
    fn list(&self) {}
}

fn main() {
    assert_eq!(
        UsersController::__nivasa_controller_api_operation_metadata(),
        vec![("list", Some("Get all users"))],
    );
}
