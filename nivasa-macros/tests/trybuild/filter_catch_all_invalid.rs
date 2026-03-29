use nivasa_macros::catch_all;

#[catch_all(HttpException)]
struct BrokenFilter;

fn main() {}
