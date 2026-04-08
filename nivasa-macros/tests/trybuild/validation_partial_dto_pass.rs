use nivasa_macros::PartialDto;
use nivasa_validation::Validate;

#[derive(PartialDto)]
struct PatchUserDto {
    #[is_email]
    email: Option<String>,
    #[min_length(6)]
    password: Option<String>,
}

fn main() {
    let dto = PatchUserDto {
        email: Some("alice@example.com".into()),
        password: None,
    };

    let _ = dto.validate();
}
