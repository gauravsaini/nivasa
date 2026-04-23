use nivasa_macros::controller;

#[controller("/users")]
#[nivasa_macros::skip_throttle(true)]
struct UsersController;

fn main() {}
