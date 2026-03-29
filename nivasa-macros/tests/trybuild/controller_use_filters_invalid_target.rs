use nivasa_macros::use_filters;

struct RequestScopedFilter;

#[use_filters(RequestScopedFilter)]
struct UsersController;

fn main() {}
