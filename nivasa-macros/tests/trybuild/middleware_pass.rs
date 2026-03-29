use std::any::TypeId;

#[nivasa_macros::middleware]
struct LoggingMiddleware;

fn main() {
    let _ = LoggingMiddleware::__nivasa_middleware_name();
    let _ = LoggingMiddleware::__nivasa_middleware_type_id();
    let _ = TypeId::of::<LoggingMiddleware>();
}
