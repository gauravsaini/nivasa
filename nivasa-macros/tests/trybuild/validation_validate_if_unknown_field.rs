use nivasa_macros::Dto;

#[derive(Dto)]
struct UnknownFieldConditionalDto {
    #[validate_if(state, "create")]
    #[is_email]
    email: String,
}

fn main() {}
