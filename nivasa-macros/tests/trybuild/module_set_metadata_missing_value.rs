use nivasa_macros::{module, set_metadata};

#[module({})]
#[set_metadata(key = "tenant")]
struct AppModule;

fn main() {}
