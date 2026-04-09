use nivasa_macros::Dto;

#[derive(Dto)]
struct InvalidMaxForm {
    #[max(5)]
    title: String,
}

fn main() {}
