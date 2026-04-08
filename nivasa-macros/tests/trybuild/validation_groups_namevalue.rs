use nivasa_macros::Dto;

#[derive(Dto)]
struct CreateForm {
    #[groups = "create"]
    #[is_email]
    email: String,
}

fn main() {}
