use nivasa_macros::controller;

#[controller("/users")]
#[nivasa_macros::throttle(limit = 5, ttl = 30)]
#[nivasa_macros::throttle(limit = 1, ttl = 10)]
struct UsersController;

fn main() {}
