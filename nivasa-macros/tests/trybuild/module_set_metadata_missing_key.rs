use nivasa_macros::{module, set_metadata};

#[module({})]
#[set_metadata(value = "billing")]
struct AppModule;

fn main() {}
