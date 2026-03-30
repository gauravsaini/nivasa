use nivasa_macros::Dto;

#[derive(Dto)]
struct Metrics {
    #[is_int]
    retry_count: String,
}

fn main() {}
