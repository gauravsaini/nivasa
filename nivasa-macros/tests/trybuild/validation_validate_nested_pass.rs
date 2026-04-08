use nivasa_macros::Dto;
use nivasa_validation::Validate;

#[derive(Dto)]
struct ContactDetails {
    #[is_email]
    email: String,
    #[min_length(6)]
    password: String,
}

#[derive(Dto)]
struct AccountForm {
    #[validate_nested]
    contact: Option<ContactDetails>,
}

fn main() {
    let form = AccountForm {
        contact: Some(ContactDetails {
            email: "alice@example.com".into(),
            password: "secret1".into(),
        }),
    };

    let _ = form.validate();
}
