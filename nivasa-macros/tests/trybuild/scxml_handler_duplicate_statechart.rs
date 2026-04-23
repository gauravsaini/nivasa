use nivasa_macros::scxml_handler;

#[scxml_handler(statechart = "request", statechart = "module", state = "guard_chain")]
fn handler() {}

fn main() {}
