use nivasa_macros::controller;
use nivasa_routing::Controller;

#[controller({ path: "/users", version: "1" })]
struct UsersV1Controller;

fn main() {
    assert_eq!(UsersV1Controller::__nivasa_controller_path(), "/users");
    assert_eq!(UsersV1Controller::__nivasa_controller_version(), Some("1"));
    assert_eq!(
        UsersV1Controller::__nivasa_controller_metadata(),
        ("/users", Some("1"))
    );

    let metadata = UsersV1Controller.metadata();
    assert_eq!(metadata.path(), "/users");
    assert_eq!(metadata.version(), Some("1"));
}
