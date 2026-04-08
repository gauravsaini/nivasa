use nivasa_macros::Dto;

#[derive(Dto)]
struct StringCustomValidator {
    #[custom_validate("validator")]
    email: String,
}

fn main() {}
