use nivasa_macros::{controller, interceptor};

#[interceptor()]
#[controller("/users")]
struct UsersController;

fn main() {}
