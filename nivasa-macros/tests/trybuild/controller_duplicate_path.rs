use nivasa_macros::controller;

#[controller({ path: "/users", path: "/admins" })]
struct UsersController;

fn main() {}
