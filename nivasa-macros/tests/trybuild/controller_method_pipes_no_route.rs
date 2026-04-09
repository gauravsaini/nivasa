use nivasa_macros::{controller, impl_controller};

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::pipe(nivasa_pipes::TrimPipe)]
    fn create(&self) {}
}

fn main() {}
