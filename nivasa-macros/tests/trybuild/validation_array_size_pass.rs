use nivasa_macros::{Dto, PartialDto};
use nivasa_validation::Validate;

#[derive(Dto)]
struct TagListForm {
    #[array_min_size(2)]
    #[array_max_size(3)]
    tags: Vec<String>,
}

#[derive(Dto)]
struct OptionalTagListForm {
    #[is_optional]
    #[array_min_size(2)]
    tags: Option<Vec<String>>,
}

#[derive(PartialDto)]
struct PartialTagListForm {
    #[array_max_size(3)]
    tags: Option<Vec<String>>,
}

fn main() {
    let form = TagListForm {
        tags: vec!["one".into(), "two".into()],
    };

    let _ = form.validate();
    let _ = OptionalTagListForm { tags: None }.validate();
    let _ = PartialTagListForm {
        tags: Some(vec!["one".into()]),
    }
    .validate();
}
