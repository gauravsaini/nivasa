use nivasa_macros::{interceptor, module};

#[module({})]
#[interceptor()]
struct AppModule;

fn main() {}
