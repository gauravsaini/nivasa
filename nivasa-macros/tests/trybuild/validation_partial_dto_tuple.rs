use nivasa_macros::PartialDto;

#[derive(PartialDto)]
struct InvalidPatchTupleDto(
    #[is_email] Option<String>,
);

fn main() {}
