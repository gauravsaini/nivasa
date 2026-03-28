use nivasa_macros::{controller, impl_controller};

struct AuthGuard;

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::guard(AuthGuard)]
    fn list(&self) {}
}

fn main() {}
