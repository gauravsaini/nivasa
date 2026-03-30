use nivasa_macros::Dto;
use nivasa_validation::Validate;

#[derive(Dto)]
struct FeatureFlags {
    #[is_boolean]
    enabled: bool,
}

fn main() {
    let form = FeatureFlags { enabled: true };

    let _ = form.validate();
}
