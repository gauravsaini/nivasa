use nivasa_macros::Dto;

#[derive(Dto)]
struct SignupForm {
    #[min_length]
    password: String,
}

fn main() {}
