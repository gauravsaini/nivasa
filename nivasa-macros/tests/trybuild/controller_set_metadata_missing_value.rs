use nivasa_macros::{controller, set_metadata};

#[set_metadata(key = "scope")]
#[controller("/users")]
struct UsersController;

fn main() {}
