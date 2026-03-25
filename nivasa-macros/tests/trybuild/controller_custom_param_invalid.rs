use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::post("/create")]
    fn create(#[nivasa_macros::custom_param()] extractor: String) {
        let _ = extractor;
    }
}

fn main() {}
