use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::get("/")]
    #[nivasa_macros::pipe()]
    fn list(&self) {}
}

fn main() {}
