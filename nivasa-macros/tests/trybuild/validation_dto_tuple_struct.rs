use nivasa_macros::Dto;

#[derive(Dto)]
struct InvalidTupleDto(
    #[is_email] String,
);

fn main() {}
