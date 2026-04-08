use nivasa_macros::Dto;

#[derive(Dto)]
struct OptionalContactForm {
    #[is_optional]
    email: String,
}

fn main() {}
