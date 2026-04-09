use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::post("/create")]
    #[nivasa_macros::pipe(nivasa_pipes::TrimPipe, nivasa_pipes::ParseBoolPipe)]
    fn create(#[nivasa_macros::body("body")] body: String) {
        let _ = body;
    }
}

fn main() {
    assert_eq!(
        UsersController::__nivasa_controller_pipe_metadata(),
        vec![(
            "create",
            vec!["nivasa_pipes::TrimPipe", "nivasa_pipes::ParseBoolPipe"],
        )],
    );
}
