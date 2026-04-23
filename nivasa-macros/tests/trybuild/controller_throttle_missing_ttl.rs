use nivasa_macros::controller;

#[controller("/users")]
#[nivasa_macros::throttle(limit = 5)]
struct UsersController;

fn main() {}
