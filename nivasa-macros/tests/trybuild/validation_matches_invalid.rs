use nivasa_macros::Dto;

#[derive(Dto)]
struct BadPatternTarget {
    #[matches("^[a-z0-9-]+$")]
    slug: u32,
}

fn main() {}
