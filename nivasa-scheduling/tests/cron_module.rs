use chrono::{TimeZone, Utc};
use nivasa_scheduling::{CronSchedule, ScheduleModule, SchedulePattern};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

#[tokio::test]
async fn cron_schedule_parses_and_computes_next_fire_time() {
    let schedule = CronSchedule::parse("0 */5 * * * *").expect("cron should parse");
    let now = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();

    let next = schedule.next_after(now).expect("next fire time");

    assert_eq!(next, Utc.with_ymd_and_hms(2024, 1, 1, 0, 5, 0).unwrap());
}

#[tokio::test]
async fn schedule_module_invokes_due_cron_jobs_and_reschedules_them() {
    let scheduler = ScheduleModule::new();
    let now = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let hits = Arc::new(AtomicUsize::new(0));

    let job_id = scheduler
        .register_cron_at("heartbeat", "0 * * * * *", now, {
            let hits = Arc::clone(&hits);
            move || {
                let hits = Arc::clone(&hits);
                async move {
                    hits.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            }
        })
        .await
        .expect("job should register");

    assert_eq!(
        scheduler.next_fire_at(job_id).await,
        Some(Utc.with_ymd_and_hms(2024, 1, 1, 0, 1, 0).unwrap())
    );

    let info = scheduler.job(job_id).await.expect("job info");
    assert_eq!(
        info.pattern,
        SchedulePattern::Cron {
            expression: "0 * * * * *".to_string()
        }
    );

    let fired = scheduler
        .tick_at(Utc.with_ymd_and_hms(2024, 1, 1, 0, 3, 0).unwrap())
        .await
        .expect("tick should run");

    assert_eq!(fired, vec![job_id, job_id, job_id]);
    assert_eq!(hits.load(Ordering::SeqCst), 3);
    assert_eq!(
        scheduler.next_fire_at(job_id).await,
        Some(Utc.with_ymd_and_hms(2024, 1, 1, 0, 4, 0).unwrap())
    );
}

#[tokio::test]
async fn schedule_module_can_remove_jobs_before_tick() {
    let scheduler = ScheduleModule::new();
    let now = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let hits = Arc::new(AtomicUsize::new(0));

    let job_id = scheduler
        .register_cron_at("ephemeral", "0 * * * * *", now, {
            let hits = Arc::clone(&hits);
            move || {
                let hits = Arc::clone(&hits);
                async move {
                    hits.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            }
        })
        .await
        .expect("job should register");

    assert!(scheduler.remove_job(job_id).await);
    assert_eq!(scheduler.job_count().await, 0);

    let fired = scheduler
        .tick_at(Utc.with_ymd_and_hms(2024, 1, 1, 0, 3, 0).unwrap())
        .await
        .expect("tick should still succeed");

    assert!(fired.is_empty());
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn schedule_pattern_constructors_match_runtime_scheduling() {
    assert_eq!(
        SchedulePattern::interval(Duration::from_millis(5)),
        SchedulePattern::Interval {
            every: Duration::from_millis(5)
        }
    );
    assert_eq!(
        SchedulePattern::timeout(Duration::from_millis(10)),
        SchedulePattern::Timeout {
            delay: Duration::from_millis(10)
        }
    );
}

#[tokio::test]
async fn interval_jobs_fire_repeatedly_and_timeout_jobs_fire_once() {
    let scheduler = ScheduleModule::new();
    let now = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let interval_hits = Arc::new(AtomicUsize::new(0));
    let timeout_hits = Arc::new(AtomicUsize::new(0));
    let interval_pattern = SchedulePattern::interval(Duration::from_millis(5));
    let timeout_pattern = SchedulePattern::timeout(Duration::from_millis(10));

    let interval_id = scheduler
        .register_pattern_at("heartbeat", interval_pattern, now, {
            let interval_hits = Arc::clone(&interval_hits);
            move || {
                let interval_hits = Arc::clone(&interval_hits);
                async move {
                    interval_hits.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            }
        })
        .await
        .expect("interval job should register");

    let timeout_id = scheduler
        .register_pattern_at("warmup", timeout_pattern, now, {
            let timeout_hits = Arc::clone(&timeout_hits);
            move || {
                let timeout_hits = Arc::clone(&timeout_hits);
                async move {
                    timeout_hits.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            }
        })
        .await
        .expect("timeout job should register");

    assert_eq!(
        scheduler.next_fire_at(interval_id).await,
        Some(
            Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap() + chrono::Duration::milliseconds(5)
        )
    );
    assert_eq!(
        scheduler.next_fire_at(timeout_id).await,
        Some(
            Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap() + chrono::Duration::milliseconds(10)
        )
    );

    let fired = scheduler
        .tick_at(
            Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap() + chrono::Duration::milliseconds(15),
        )
        .await
        .expect("tick should run");

    assert_eq!(timeout_hits.load(Ordering::SeqCst), 1);
    assert!(interval_hits.load(Ordering::SeqCst) >= 3);
    assert!(fired.contains(&timeout_id));
    assert!(fired.iter().filter(|&&id| id == interval_id).count() >= 3);
    assert!(scheduler.job(timeout_id).await.is_none());
    assert_eq!(
        scheduler.next_fire_at(interval_id).await,
        Some(
            Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap() + chrono::Duration::milliseconds(20)
        )
    );
}
