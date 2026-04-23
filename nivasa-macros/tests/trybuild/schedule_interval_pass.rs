extern crate self as nivasa_scheduling;

use nivasa_macros::interval;

#[derive(Debug, PartialEq, Eq)]
pub enum SchedulePattern {
    Interval { every: std::time::Duration },
}

impl SchedulePattern {
    pub fn interval(every: std::time::Duration) -> Self {
        Self::Interval { every }
    }
}

struct Jobs;

impl Jobs {
    #[interval(5000)]
    fn tick(&self) {}
}

fn main() {
    let jobs = Jobs;
    jobs.tick();

    assert_eq!(
        Jobs::__nivasa_interval_metadata_for_tick(),
        SchedulePattern::Interval {
            every: std::time::Duration::from_millis(5000),
        },
    );
}
