use nivasa_macros::scxml_handler;

#[scxml_handler(statechart = "request/../../evil", state = "guard_chain")]
fn guarded_handler() {}

fn main() {}
