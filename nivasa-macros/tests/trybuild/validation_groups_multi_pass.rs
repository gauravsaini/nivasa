use nivasa_macros::Dto;
use nivasa_validation::{Validate, ValidationContext};

#[derive(Dto)]
struct GroupedForm {
    #[groups("create", "update")]
    #[min_length(6)]
    password: String,
}

fn main() {
    let form = GroupedForm {
        password: "secret1".into(),
    };

    let _ = form.validate_with(&ValidationContext::new().with_group("update"));
}
