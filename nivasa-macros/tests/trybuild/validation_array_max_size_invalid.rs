use nivasa_macros::Dto;

#[derive(Dto)]
struct TagListForm {
    #[array_max_size(3)]
    tags: bool,
}

fn main() {}
