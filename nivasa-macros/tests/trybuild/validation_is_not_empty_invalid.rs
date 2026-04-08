use nivasa_macros::Dto;

#[derive(Dto)]
struct InvalidContentForm {
    #[is_not_empty]
    title: u32,
}

fn main() {}
