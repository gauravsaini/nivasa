use nivasa_macros::Dto;

#[derive(Dto)]
struct CreateForm {
    #[groups(123)]
    #[is_email]
    email: String,
}

fn main() {}
