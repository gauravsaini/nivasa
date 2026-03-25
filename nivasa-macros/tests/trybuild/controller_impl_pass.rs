use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::get("/list")]
    fn list(&self) {}

    #[nivasa_macros::post("create")]
    fn create(&self) {}
}

fn main() {
    assert_eq!(
        UsersController::__nivasa_controller_routes(),
        vec![
            ("GET", "/users/list".to_string(), "list"),
            ("POST", "/users/create".to_string(), "create"),
        ],
    );
}
