use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    fn create(#[nivasa_macros::pipe(nivasa_pipes::TrimPipe)] body: String) {
        let _ = body;
    }
}

fn main() {}
