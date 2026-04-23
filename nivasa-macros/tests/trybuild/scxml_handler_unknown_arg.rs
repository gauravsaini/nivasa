use nivasa_macros::scxml_handler;

#[scxml_handler(statechart = "request", state = "guard_chain", mode = "strict")]
fn handler() {}

fn main() {}
