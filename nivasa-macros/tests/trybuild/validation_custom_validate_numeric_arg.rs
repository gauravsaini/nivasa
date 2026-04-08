use nivasa_macros::Dto;

#[derive(Dto)]
struct NumericCustomValidator {
    #[custom_validate(42)]
    email: String,
}

fn main() {}
