use nivasa_macros::use_filters;

struct RequestScopedFilter;

#[use_filters(RequestScopedFilter)]
enum NotAController {}

fn main() {}
