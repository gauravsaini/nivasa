use nivasa_macros::ConfigSchema;

#[derive(ConfigSchema)]
struct TupleConfig(String, String);

fn main() {}
