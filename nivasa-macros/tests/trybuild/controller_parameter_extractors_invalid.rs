use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::post("/create")]
    fn create(#[nivasa_macros::param("")] id: String) {
        let _ = id;
    }
}

fn main() {}
