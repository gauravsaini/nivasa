use nivasa_macros::{controller, impl_controller, roles};

#[roles("admin", "editor")]
#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[roles("reader")]
    #[nivasa_macros::get("/list")]
    fn list(&self) {}
}

fn main() {
    assert_eq!(
        UsersController::__nivasa_controller_roles(),
        vec!["admin", "editor"],
    );
    assert_eq!(
        UsersController::__nivasa_controller_role_metadata(),
        vec![("list", vec!["reader"])],
    );
}
