use nivasa_macros::ConfigSchema as DeriveConfigSchema;

pub trait ConfigSchema {
    fn required_keys() -> &'static [&'static str] {
        &[]
    }

    fn defaults() -> &'static [(&'static str, &'static str)] {
        &[]
    }
}

#[derive(DeriveConfigSchema)]
struct AppConfig {
    host: String,
    #[schema(default = "3000")]
    port: String,
}

fn main() {
    assert_eq!(AppConfig::required_keys(), &["host"]);
    assert_eq!(AppConfig::defaults(), &[("port", "3000")]);
}
