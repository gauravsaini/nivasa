use nivasa_macros::scxml_handler;

#[scxml_handler(statechart = "request", state = "../guard_chain")]
fn handler() {}

fn main() {}
