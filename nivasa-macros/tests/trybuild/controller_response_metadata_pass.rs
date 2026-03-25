use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::post("/create")]
    #[nivasa_macros::http_code(201)]
    #[nivasa_macros::header("x-powered-by", "nivasa")]
    #[nivasa_macros::header("cache-control", "no-store")]
    fn create(&self) {}
}

fn main() {
    assert_eq!(
        UsersController::__nivasa_controller_response_metadata(),
        vec![(
            "create",
            Some(201),
            vec![
                ("x-powered-by", "nivasa"),
                ("cache-control", "no-store"),
            ],
        )],
    );
}
