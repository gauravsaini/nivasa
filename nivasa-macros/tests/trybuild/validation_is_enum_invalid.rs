use nivasa_macros::Dto;

#[derive(Dto)]
struct AccessForm {
    #[is_enum(AccessLevel)]
    access_level: bool,
}

fn main() {}
