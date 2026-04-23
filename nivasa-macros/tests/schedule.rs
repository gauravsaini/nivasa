use nivasa_macros::{cron, interval, timeout};
use nivasa_scheduling::SchedulePattern;
use std::time::Duration;
use trybuild::TestCases;

struct RuntimeJobs;

impl RuntimeJobs {
    #[cron("0 */5 * * * *")]
    fn cron_tick(&self) {}

    #[interval(250)]
    fn interval_tick(&self) {}

    #[timeout(750)]
    fn timeout_tick(&self) {}
}

#[test]
fn schedule_macros_emit_runtime_metadata_helpers() {
    assert_eq!(
        RuntimeJobs::__nivasa_cron_metadata_for_cron_tick(),
        SchedulePattern::Cron {
            expression: "0 */5 * * * *".to_string()
        }
    );
    assert_eq!(
        RuntimeJobs::__nivasa_interval_metadata_for_interval_tick(),
        SchedulePattern::Interval {
            every: Duration::from_millis(250)
        }
    );
    assert_eq!(
        RuntimeJobs::__nivasa_timeout_metadata_for_timeout_tick(),
        SchedulePattern::Timeout {
            delay: Duration::from_millis(750)
        }
    );

    let jobs = RuntimeJobs;
    jobs.cron_tick();
    jobs.interval_tick();
    jobs.timeout_tick();
}

#[test]
fn schedule_macro_validation() {
    let t = TestCases::new();
    t.pass("tests/trybuild/schedule_cron_pass.rs");
    t.pass("tests/trybuild/schedule_interval_pass.rs");
    t.pass("tests/trybuild/schedule_timeout_pass.rs");
    t.compile_fail("tests/trybuild/schedule_interval_negative.rs");
    t.compile_fail("tests/trybuild/schedule_cron_empty.rs");
    t.compile_fail("tests/trybuild/schedule_cron_invalid_literal.rs");
    t.compile_fail("tests/trybuild/schedule_interval_static_method.rs");
    t.compile_fail("tests/trybuild/schedule_interval_zero.rs");
    t.compile_fail("tests/trybuild/schedule_timeout_invalid_literal.rs");
    t.compile_fail("tests/trybuild/schedule_timeout_invalid_target.rs");
}
