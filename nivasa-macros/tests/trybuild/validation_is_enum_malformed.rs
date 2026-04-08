use nivasa_macros::Dto;

#[derive(Dto)]
struct AccessForm {
    #[is_enum]
    access_level: String,
}

fn main() {}
