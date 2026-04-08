use nivasa_macros::Dto;

#[derive(Dto)]
struct FeatureFlags {
    #[is_boolean]
    enabled: String,
}

fn main() {}
