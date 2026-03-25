use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    fn create(#[nivasa_macros::headers("headers")] headers: String) {
        let _ = headers;
    }
}

fn main() {}
