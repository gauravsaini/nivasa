use chrono::{Duration as ChronoDuration, TimeZone, Utc};
use nivasa_scheduling::{CronSchedule, ScheduleError, ScheduleModule, SchedulePattern};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;
use uuid::Uuid;

#[tokio::test]
async fn cron_schedule_parses_and_computes_next_fire_time() {
    let schedule = CronSchedule::parse("0 */5 * * * *").expect("cron should parse");
    let now = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();

    let next = schedule.next_after(now).expect("next fire time");

    assert_eq!(next, Utc.with_ymd_and_hms(2024, 1, 1, 0, 5, 0).unwrap());
}

#[tokio::test]
async fn cron_schedule_reports_invalid_expressions() {
    let error = CronSchedule::parse("not-a-cron").expect_err("invalid cron should fail");

    match error {
        ScheduleError::InvalidCronExpression {
            expression,
            message,
        } => {
            assert_eq!(expression, "not-a-cron");
            assert!(!message.is_empty());
        }
        other => panic!("unexpected error: {other:?}"),
    }
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

#[tokio::test]
async fn schedule_module_rejects_invalid_interval_and_overflowing_patterns() {
    let scheduler = ScheduleModule::new();
    let now = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();

    let zero_interval = scheduler
        .register_interval_at("zero", Duration::ZERO, now, || async { Ok(()) })
        .await
        .expect_err("zero interval should fail");
    assert_eq!(
        zero_interval,
        ScheduleError::InvalidIntervalDuration { every_ms: 0 }
    );

    let huge_interval = scheduler
        .register_pattern_at(
            "huge-interval",
            SchedulePattern::interval(Duration::MAX),
            now,
            || async { Ok(()) },
        )
        .await
        .expect_err("overflowing interval should fail");
    assert_eq!(
        huge_interval,
        ScheduleError::NoUpcomingFireTime {
            expression: format!("interval:{:?}", Duration::MAX),
        }
    );

    let huge_timeout = scheduler
        .register_timeout_at("huge-timeout", Duration::MAX, now, || async { Ok(()) })
        .await
        .expect_err("overflowing timeout should fail");
    assert_eq!(
        huge_timeout,
        ScheduleError::NoUpcomingFireTime {
            expression: format!("timeout:{:?}", Duration::MAX),
        }
    );
}

#[tokio::test]
async fn tick_at_returns_job_failed_and_stops_later_due_jobs() {
    let scheduler = ScheduleModule::new();
    let now = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let healthy_hits = Arc::new(AtomicUsize::new(0));

    let failing_timeout_id = scheduler
        .register_timeout_at("failing-timeout", Duration::from_millis(10), now, || async {
            Err("boom".to_string())
        })
        .await
        .expect("timeout job should register");

    let healthy_interval_id = scheduler
        .register_interval_at("healthy-interval", Duration::from_millis(20), now, {
            let healthy_hits = Arc::clone(&healthy_hits);
            move || {
                let healthy_hits = Arc::clone(&healthy_hits);
                async move {
                    healthy_hits.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            }
        })
        .await
        .expect("interval job should register");

    let error = scheduler
        .tick_at(now + ChronoDuration::milliseconds(20))
        .await
        .expect_err("failing timeout should bubble up");
    assert_eq!(
        error,
        ScheduleError::JobFailed {
            job_name: "failing-timeout".to_string(),
            message: "boom".to_string(),
        }
    );

    assert_eq!(healthy_hits.load(Ordering::SeqCst), 0);
    assert!(scheduler.job(failing_timeout_id).await.is_none());
    assert_eq!(
        scheduler.next_fire_at(healthy_interval_id).await,
        Some(now + ChronoDuration::milliseconds(20))
    );
}

#[tokio::test]
async fn schedule_module_job_helpers_cover_missing_and_snapshot_paths() {
    let scheduler = ScheduleModule::new();
    let now = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();

    let interval_id = scheduler
        .register_interval_at("heartbeat", Duration::from_secs(5), now, || async { Ok(()) })
        .await
        .expect("interval job should register");

    let timeout_id = scheduler
        .register_timeout_at("warmup", Duration::from_secs(2), now, || async { Ok(()) })
        .await
        .expect("timeout job should register");

    assert_eq!(scheduler.job_count().await, 2);
    assert!(!scheduler.remove_job(Uuid::new_v4()).await);

    let mut jobs = scheduler.jobs().await;
    jobs.sort_by(|left, right| left.name.cmp(&right.name));
    assert_eq!(jobs.len(), 2);
    assert_eq!(jobs[0].id, interval_id);
    assert_eq!(
        jobs[0].pattern,
        SchedulePattern::Interval {
            every: Duration::from_secs(5),
        }
    );
    assert_eq!(jobs[1].id, timeout_id);
    assert_eq!(
        jobs[1].pattern,
        SchedulePattern::Timeout {
            delay: Duration::from_secs(2),
        }
    );

    let fired = scheduler
        .tick_at(now + ChronoDuration::seconds(1))
        .await
        .expect("no jobs should be due yet");
    assert!(fired.is_empty());
}
