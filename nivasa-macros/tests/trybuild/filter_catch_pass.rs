use nivasa_macros::catch;

struct HttpException;

#[catch(HttpException)]
struct HttpExceptionFilter;

fn main() {
    assert_eq!(
        HttpExceptionFilter::__nivasa_filter_exception(),
        "HttpException",
    );
}
