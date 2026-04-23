use nivasa_macros::{controller, set_metadata};

#[set_metadata(value = "users")]
#[controller("/users")]
struct UsersController;

fn main() {}
