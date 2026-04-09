use nivasa_macros::{controller, impl_controller};

struct CreateUserDto;

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::post("/")]
    #[nivasa_macros::api_body(type = CreateUserDto)]
    fn create(&self) {}
}

fn main() {
    assert_eq!(
        UsersController::__nivasa_controller_api_body_metadata(),
        vec![("create", "CreateUserDto")],
    );
}
