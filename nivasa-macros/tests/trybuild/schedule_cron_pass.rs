use nivasa_macros::cron;

extern crate self as nivasa_scheduling;

#[derive(Debug, PartialEq, Eq)]
pub enum SchedulePattern {
    Cron { expression: String },
}

impl SchedulePattern {
    pub fn cron(expression: impl Into<String>) -> Self {
        Self::Cron {
            expression: expression.into(),
        }
    }
}

struct Jobs;

impl Jobs {
    #[cron("0 */5 * * * *")]
    fn tick(&self) {}
}

fn main() {
    let jobs = Jobs;
    jobs.tick();

    assert_eq!(
        Jobs::__nivasa_cron_metadata_for_tick(),
        SchedulePattern::Cron {
            expression: "0 */5 * * * *".to_string(),
        },
    );
}
