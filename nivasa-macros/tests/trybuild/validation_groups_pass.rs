use nivasa_macros::Dto;
use nivasa_validation::{Validate, ValidationContext};

#[derive(Dto)]
struct CreateForm {
    #[groups("create")]
    #[is_email]
    email: String,
}

fn main() {
    let form = CreateForm {
        email: "alice@example.com".into(),
    };

    let _ = form.validate();
    let _ = form.validate_with(&ValidationContext::new().with_group("create"));
}
