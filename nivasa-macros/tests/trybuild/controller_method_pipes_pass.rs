use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::post("/create")]
    #[nivasa_macros::pipe(nivasa_pipes::TrimPipe)]
    fn create(#[nivasa_macros::body("body")] body: String) {
        let _ = body;
    }
}

fn main() {
    assert_eq!(
        UsersController::__nivasa_controller_pipe_metadata(),
        vec![("create", vec!["nivasa_pipes::TrimPipe"])],
    );
    assert_eq!(
        UsersController::__nivasa_controller_parameter_metadata(),
        vec![("create", vec![("body", Some("body"))])],
    );
    assert_eq!(
        UsersController::__nivasa_controller_parameter_pipe_metadata(),
        Vec::<(&'static str, Vec<Option<&'static str>>)>::new(),
    );
}
