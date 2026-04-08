use nivasa_macros::Dto;

#[derive(Dto)]
struct CreateForm {
    #[groups("create", 123)]
    #[is_email]
    email: String,
}

fn main() {}
