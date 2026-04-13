use nivasa_macros::ConfigSchema;

#[derive(ConfigSchema)]
struct BadConfig {
    #[schema(foo = "bar")]
    host: String,
}

fn main() {}
