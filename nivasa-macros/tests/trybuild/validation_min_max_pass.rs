use nivasa_macros::Dto;
use nivasa_validation::Validate;

#[derive(Dto)]
struct MetricForm {
    #[min(2)]
    #[max(5)]
    retry_count: u32,
    #[min(-10.5)]
    #[max(20.25)]
    average_latency_ms: f64,
    #[is_optional]
    #[min(2)]
    #[max(5)]
    optional_retry_count: Option<u32>,
}

fn main() {
    let form = MetricForm {
        retry_count: 3,
        average_latency_ms: 12.5,
        optional_retry_count: None,
    };

    let _ = form.validate();
}
