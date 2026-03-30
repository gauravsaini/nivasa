use nivasa_macros::{controller, impl_controller, set_metadata};

#[set_metadata(key = "controller", value = "UsersController")]
#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[set_metadata(key = "trace", value = "enabled")]
    #[nivasa_macros::get("/list")]
    fn list(&self) {}
}

fn main() {
    assert_eq!(
        UsersController::__nivasa_controller_set_metadata(),
        vec![("controller", "UsersController")],
    );
    assert_eq!(
        UsersController::__nivasa_controller_set_metadata_metadata(),
        vec![("list", vec![("trace", "enabled")])],
    );
}
