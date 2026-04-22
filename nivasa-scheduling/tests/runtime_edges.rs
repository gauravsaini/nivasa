use chrono::{TimeZone, Utc};
use nivasa_scheduling::{CronSchedule, ScheduleError, ScheduleModule};

#[tokio::test]
async fn register_cron_at_rejects_cron_without_upcoming_fire_time() {
    let scheduler = ScheduleModule::new();
    let now = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let expression = "0 0 0 1 1 * 2020";

    let schedule = CronSchedule::parse(expression).expect("cron with explicit year should parse");
    assert_eq!(schedule.expression(), expression);
    assert_eq!(schedule.next_after(now), None);

    let error = scheduler
        .register_cron_at("past-once", expression, now, || async { Ok(()) })
        .await
        .expect_err("past-only cron should not register");

    assert_eq!(
        error,
        ScheduleError::NoUpcomingFireTime {
            expression: expression.to_string(),
        }
    );
    assert_eq!(scheduler.job_count().await, 0);
}
