use nivasa_macros::{controller, impl_controller};

#[nivasa_macros::pipe(nivasa_pipes::TrimPipe, nivasa_pipes::ParseBoolPipe)]
#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::get("/")]
    fn list(&self) {}
}

fn main() {
    assert_eq!(
        UsersController::__nivasa_controller_pipes(),
        vec!["nivasa_pipes::TrimPipe", "nivasa_pipes::ParseBoolPipe"],
    );
}
