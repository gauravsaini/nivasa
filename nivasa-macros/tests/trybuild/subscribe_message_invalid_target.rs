use nivasa_macros::subscribe_message;

#[subscribe_message("chat.join")]
fn not_a_method() {}

fn main() {}
