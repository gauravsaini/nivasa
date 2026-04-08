use nivasa_macros::Dto;
use nivasa_validation::{Validate, ValidationContext};

#[derive(Dto)]
struct ContactDetails {
    #[groups("create")]
    #[is_email]
    email: String,
}

#[derive(Dto)]
struct AccountForm {
    #[validate_nested]
    contact: ContactDetails,
}

fn main() {
    let form = AccountForm {
        contact: ContactDetails {
            email: "alice@example.com".into(),
        },
    };

    let _ = form.validate_with(&ValidationContext::new().with_group("create"));
}
