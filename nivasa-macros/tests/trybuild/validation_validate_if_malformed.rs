use nivasa_macros::Dto;

#[derive(Dto)]
struct MalformedConditionalDto {
    #[validate_if]
    #[is_email]
    email: String,
}

fn main() {}
