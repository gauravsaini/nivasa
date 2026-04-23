use nivasa_macros::{controller, use_filters};

#[use_filters()]
#[controller("/users")]
struct UsersController;

fn main() {}
