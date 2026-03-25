use nivasa_macros::scxml_handler;

#[scxml_handler(statechart = "does_not_exist", state = "guard_chain")]
fn guarded_handler() {}

fn main() {}
