use nivasa_macros::Dto;

#[derive(Dto)]
struct CreateForm {
    #[groups()]
    #[is_email]
    email: String,
}

fn main() {}
