use nivasa_macros::Dto;

#[derive(Dto)]
struct InvalidSourceTypeConditionalDto {
    state: bool,
    #[validate_if(state, "true")]
    #[is_email]
    email: String,
}

fn main() {}
