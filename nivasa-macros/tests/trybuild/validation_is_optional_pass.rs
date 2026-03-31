use nivasa_macros::Dto;
use nivasa_validation::Validate;

#[derive(Dto)]
struct OptionalContactForm {
    #[is_optional]
    #[is_email]
    email: Option<String>,
}

fn main() {
    let form = OptionalContactForm { email: None };
    let _ = form.validate();
}
