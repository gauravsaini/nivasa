use nivasa_macros::controller;

#[controller("/users")]
struct UsersController;

fn main() {
    assert_eq!(UsersController::__nivasa_controller_path(), "/users");
    assert_eq!(UsersController::__nivasa_controller_version(), None);
    assert_eq!(
        UsersController::__nivasa_controller_metadata(),
        ("/users", None)
    );
}
