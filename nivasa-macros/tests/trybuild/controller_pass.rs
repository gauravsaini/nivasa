use nivasa_macros::controller;
use nivasa_routing::Controller;

#[controller("/users")]
struct UsersController;

fn main() {
    assert_eq!(UsersController::__nivasa_controller_path(), "/users");
    assert_eq!(UsersController::__nivasa_controller_version(), None);
    assert_eq!(
        UsersController::__nivasa_controller_metadata(),
        ("/users", None)
    );

    let metadata = UsersController.metadata();
    assert_eq!(metadata.path(), "/users");
    assert_eq!(metadata.version(), None);
}
