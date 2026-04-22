use nivasa_macros::controller;

#[controller("/users", version = "1", extra = "2")]
struct UsersController;

fn main() {}
