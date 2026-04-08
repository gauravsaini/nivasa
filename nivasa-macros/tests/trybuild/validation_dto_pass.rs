use nivasa_macros::Dto;
use nivasa_validation::Validate;

#[derive(Dto)]
struct SignupForm {
    #[is_email]
    email: String,
    #[min_length(6)]
    password: String,
}

fn main() {
    let form = SignupForm {
        email: "alice@example.com".into(),
        password: "secret1".into(),
    };

    let _ = form.validate();
}
