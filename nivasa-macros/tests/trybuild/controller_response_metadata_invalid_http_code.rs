use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::post("/create")]
    #[nivasa_macros::http_code(99)]
    fn create(&self) {}
}

fn main() {}
