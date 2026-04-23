use nivasa_macros::{controller, roles};

#[roles()]
#[controller("/users")]
struct UsersController;

fn main() {}
