extern crate self as nivasa_scheduling;

use nivasa_macros::timeout;

#[derive(Debug, PartialEq, Eq)]
pub enum SchedulePattern {
    Timeout { delay: std::time::Duration },
}

impl SchedulePattern {
    pub fn timeout(delay: std::time::Duration) -> Self {
        Self::Timeout { delay }
    }
}

struct Jobs;

impl Jobs {
    #[timeout(3000)]
    fn tick(&self) {}
}

fn main() {
    let jobs = Jobs;
    jobs.tick();

    assert_eq!(
        Jobs::__nivasa_timeout_metadata_for_tick(),
        SchedulePattern::Timeout {
            delay: std::time::Duration::from_millis(3000),
        },
    );
}
