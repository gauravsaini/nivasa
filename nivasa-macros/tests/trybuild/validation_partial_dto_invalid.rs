use nivasa_macros::PartialDto;

#[derive(PartialDto)]
struct InvalidPatchUserDto {
    #[is_email]
    email: String,
}

fn main() {}
