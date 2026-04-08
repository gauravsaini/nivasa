use nivasa_macros::Dto;

#[derive(Dto)]
struct ProfileForm {
    #[max_length]
    bio: String,
}

fn main() {}
