use nivasa_macros::catch;

#[catch(HttpException)]
fn not_a_filter() {}

fn main() {}
