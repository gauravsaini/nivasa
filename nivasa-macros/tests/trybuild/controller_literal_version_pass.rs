use nivasa_macros::controller;
use nivasa_routing::Controller;

#[controller("/users", version = "2")]
struct UsersV2Controller;

fn main() {
    assert_eq!(UsersV2Controller::__nivasa_controller_path(), "/users");
    assert_eq!(UsersV2Controller::__nivasa_controller_version(), Some("2"));
    assert_eq!(
        UsersV2Controller::__nivasa_controller_metadata(),
        ("/users", Some("2"))
    );

    let metadata = UsersV2Controller.metadata();
    assert_eq!(metadata.path(), "/users");
    assert_eq!(metadata.version(), Some("2"));
}
