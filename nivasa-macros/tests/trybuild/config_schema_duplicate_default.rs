use nivasa_macros::ConfigSchema;

#[derive(ConfigSchema)]
struct BadConfig {
    #[schema(default = "3000", default = "4000")]
    port: String,
}

fn main() {}
