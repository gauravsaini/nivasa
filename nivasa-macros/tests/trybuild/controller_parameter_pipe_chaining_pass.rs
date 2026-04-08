use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::post("/create")]
    fn create(
        #[nivasa_macros::body("body")]
        #[nivasa_macros::pipe(nivasa_pipes::TrimPipe, nivasa_pipes::ParseIntPipe::<i32>)]
        body: String,
        plain: String,
        #[nivasa_macros::pipe(nivasa_pipes::TrimPipe, nivasa_pipes::ParseUuidPipe)]
        id: String,
    ) {
        let _ = (body, plain, id);
    }
}

fn main() {
    assert_eq!(
        UsersController::__nivasa_controller_parameter_pipe_metadata(),
        vec![(
            "create",
            vec![
                vec![
                    "nivasa_pipes::TrimPipe",
                    "nivasa_pipes::ParseIntPipe::<i32>",
                ],
                vec![],
                vec![
                    "nivasa_pipes::TrimPipe",
                    "nivasa_pipes::ParseUuidPipe",
                ],
            ],
        )],
    );
}
