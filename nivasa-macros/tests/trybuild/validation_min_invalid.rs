use nivasa_macros::Dto;

#[derive(Dto)]
struct InvalidMinForm {
    #[min(2)]
    title: String,
}

fn main() {}
