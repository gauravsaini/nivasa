use chrono::{TimeZone, Utc};
use nivasa_scheduling::{CronSchedule, ScheduleError, ScheduleModule};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

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

#[tokio::test]
async fn tick_at_keeps_running_when_due_job_removes_itself() {
    let scheduler = ScheduleModule::new();
    let now = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let job_id_slot = Arc::new(Mutex::new(None));

    let job_id = scheduler
        .register_interval_at("self-removing", Duration::from_secs(1), now, {
            let scheduler = scheduler.clone();
            let hits = Arc::clone(&hits);
            let job_id_slot = Arc::clone(&job_id_slot);
            move || {
                let scheduler = scheduler.clone();
                let hits = Arc::clone(&hits);
                let job_id = *job_id_slot.lock().expect("job id slot lock");
                async move {
                    hits.fetch_add(1, Ordering::SeqCst);
                    assert!(
                        scheduler
                            .remove_job(job_id.expect("job id stored before tick"))
                            .await
                    );
                    Ok(())
                }
            }
        })
        .await
        .expect("interval job should register");
    *job_id_slot.lock().expect("job id slot lock") = Some(job_id);

    let fired = scheduler
        .tick_at(Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 1).unwrap())
        .await
        .expect("self-removing job should still tick successfully");

    assert_eq!(fired, vec![job_id]);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert!(scheduler.job(job_id).await.is_none());
    assert_eq!(scheduler.next_fire_at(job_id).await, None);
    assert_eq!(scheduler.job_count().await, 0);
}
