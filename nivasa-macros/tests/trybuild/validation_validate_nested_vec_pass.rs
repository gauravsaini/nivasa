use nivasa_macros::Dto;
use nivasa_validation::Validate;

#[derive(Dto)]
struct ContactDetails {
    #[is_email]
    email: String,
}

#[derive(Dto)]
struct ContactBook {
    #[validate_nested]
    contacts: Vec<ContactDetails>,
}

fn main() {
    let form = ContactBook {
        contacts: vec![ContactDetails {
            email: "alice@example.com".into(),
        }],
    };

    let _ = form.validate();
}
