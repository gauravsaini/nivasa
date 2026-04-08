use nivasa_macros::Dto;

#[derive(Dto)]
struct TagListForm {
    #[array_min_size(2)]
    tags: bool,
}

fn main() {}
