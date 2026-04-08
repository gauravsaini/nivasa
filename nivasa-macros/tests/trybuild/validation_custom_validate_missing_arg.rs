use nivasa_macros::Dto;

#[derive(Dto)]
struct MissingCustomValidator {
    #[custom_validate()]
    email: String,
}

fn main() {}
