use nivasa_macros::{controller, skip_throttle};

#[controller("/users")]
#[skip_throttle]
#[skip_throttle]
struct UsersController;

fn main() {}
