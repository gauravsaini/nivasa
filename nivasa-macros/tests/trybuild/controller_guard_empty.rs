use nivasa_macros::{controller, guard};

#[guard()]
#[controller("/users")]
struct UsersController;

fn main() {}
