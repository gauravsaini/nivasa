use nivasa_macros::Dto;

#[derive(Dto)]
struct Metrics {
    #[is_number]
    retry_count: String,
}

fn main() {}
