use nivasa_macros::{controller, impl_controller};

struct RequestScopedFilter;

#[controller("/users")]
struct UsersController;

#[impl_controller]
impl UsersController {
    #[nivasa_macros::use_filters(RequestScopedFilter)]
    fn list(&self) {}
}

fn main() {}
