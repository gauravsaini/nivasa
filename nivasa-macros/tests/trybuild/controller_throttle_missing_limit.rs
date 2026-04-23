use nivasa_macros::controller;

#[controller("/users")]
#[nivasa_macros::throttle(ttl = 30)]
struct UsersController;

fn main() {}
