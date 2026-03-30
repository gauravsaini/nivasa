use nivasa_macros::catch_all;

#[catch_all]
struct DefaultExceptionFilter;

fn main() {
    assert!(DefaultExceptionFilter::__nivasa_filter_catch_all());
}
