use nivasa_macros::Dto;

#[derive(Dto)]
struct ContactDetails {
    email: String,
}

#[derive(Dto)]
struct AccountForm {
    #[validate_nested(extra)]
    contact: ContactDetails,
}

fn main() {}
