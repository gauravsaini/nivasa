use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::http_code(204)]
    #[nivasa_macros::header("x-controller-version", "v1")]
    fn summary(&self) {}
}

fn main() {}
