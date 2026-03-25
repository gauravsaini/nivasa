use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::post("/create")]
    fn create(#[nivasa_macros::req = "request"] request: String) {
        let _ = request;
    }
}

fn main() {}
