use nivasa_macros::scxml_handler;

#[scxml_handler(statechart = "request", state = "definitely_missing")]
fn guarded_handler() {}

fn main() {}
