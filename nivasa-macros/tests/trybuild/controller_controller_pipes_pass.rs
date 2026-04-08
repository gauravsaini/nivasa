use nivasa_macros::{controller, impl_controller};

#[nivasa_macros::pipe(nivasa_pipes::TrimPipe)]
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
        vec!["nivasa_pipes::TrimPipe"],
    );
    assert_eq!(
        UsersController::__nivasa_controller_pipe_metadata(),
        Vec::<(&'static str, Vec<&'static str>)>::new(),
    );
}
