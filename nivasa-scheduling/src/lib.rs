//! # nivasa-scheduling
//!
//! Nivasa framework - scheduling.
//!
//! This crate exposes a small in-memory scheduler that can drive cron,
//! interval, and timeout jobs against a caller-controlled clock.

use async_trait::async_trait;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use cron::Schedule;
use nivasa_core::di::container::DependencyContainer;
use nivasa_core::di::error::DiError;
use nivasa_core::di::provider::Injectable;
use std::any::TypeId;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

type JobFuture = Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'static>>;
type JobHandler = Arc<dyn Fn() -> JobFuture + Send + Sync + 'static>;

/// Identifier assigned to a registered scheduled job.
pub type ScheduleJobId = Uuid;

/// Errors raised by the scheduling runtime.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ScheduleError {
    /// The cron expression could not be parsed.
    #[error("failed to parse cron expression `{expression}`: {message}")]
    InvalidCronExpression { expression: String, message: String },
    /// The cron expression produced no future fire times.
    #[error("cron expression `{expression}` has no upcoming fire time")]
    NoUpcomingFireTime { expression: String },
    /// A registered job could not be found.
    #[error("scheduled job `{job_id}` not found")]
    JobNotFound { job_id: ScheduleJobId },
    /// A job callback returned an application error.
    #[error("scheduled job `{job_name}` failed: {message}")]
    JobFailed { job_name: String, message: String },
    /// An interval duration was invalid.
    #[error("interval duration must be greater than zero: {every_ms}ms")]
    InvalidIntervalDuration { every_ms: u64 },
}

/// Parsed cron expression wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronSchedule {
    expression: String,
    schedule: Schedule,
}

impl CronSchedule {
    /// Parse a cron expression using the `cron` crate.
    pub fn parse(expression: impl Into<String>) -> Result<Self, ScheduleError> {
        let expression = expression.into();
        let schedule = Schedule::from_str(&expression).map_err(|err| {
            ScheduleError::InvalidCronExpression {
                expression: expression.clone(),
                message: err.to_string(),
            }
        })?;

        Ok(Self {
            expression,
            schedule,
        })
    }

    /// Return the original cron expression.
    pub fn expression(&self) -> &str {
        &self.expression
    }

    /// Return the next fire time after `after`.
    pub fn next_after(&self, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.schedule.after(&after).next()
    }
}

/// Public schedule pattern snapshot for a registered job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulePattern {
    /// Cron-based trigger.
    Cron { expression: String },
    /// Repeating interval trigger.
    Interval { every: std::time::Duration },
    /// One-shot timeout trigger.
    Timeout { delay: std::time::Duration },
}

impl SchedulePattern {
    /// Create a cron pattern snapshot.
    pub fn cron(expression: impl Into<String>) -> Self {
        Self::Cron {
            expression: expression.into(),
        }
    }

    /// Create an interval pattern snapshot.
    pub fn interval(every: std::time::Duration) -> Self {
        Self::Interval { every }
    }

    /// Create a timeout pattern snapshot.
    pub fn timeout(delay: std::time::Duration) -> Self {
        Self::Timeout { delay }
    }
}

#[derive(Clone)]
enum ScheduledJobSchedule {
    Cron {
        expression: String,
        schedule: Box<CronSchedule>,
    },
    Interval {
        every: std::time::Duration,
    },
    Timeout {
        delay: std::time::Duration,
    },
}

impl ScheduledJobSchedule {
    fn initial_fire_at(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            Self::Cron { schedule, .. } => schedule.next_after(now),
            Self::Interval { every } => now.checked_add_signed(duration_to_chrono(*every)?),
            Self::Timeout { delay } => now.checked_add_signed(duration_to_chrono(*delay)?),
        }
    }

    fn next_after(&self, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            Self::Cron { schedule, .. } => schedule.next_after(after),
            Self::Interval { every } => after.checked_add_signed(duration_to_chrono(*every)?),
            Self::Timeout { .. } => None,
        }
    }

    fn pattern(&self) -> SchedulePattern {
        match self {
            Self::Cron { expression, .. } => SchedulePattern::cron(expression.clone()),
            Self::Interval { every } => SchedulePattern::interval(*every),
            Self::Timeout { delay } => SchedulePattern::timeout(*delay),
        }
    }

    fn from_pattern(pattern: SchedulePattern) -> Result<Self, ScheduleError> {
        match pattern {
            SchedulePattern::Cron { expression } => Ok(Self::Cron {
                schedule: Box::new(CronSchedule::parse(expression.clone())?),
                expression,
            }),
            SchedulePattern::Interval { every } => Ok(Self::Interval { every }),
            SchedulePattern::Timeout { delay } => Ok(Self::Timeout { delay }),
        }
    }
}

/// Public snapshot for a scheduled job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledJobInfo {
    pub id: ScheduleJobId,
    pub name: String,
    pub pattern: SchedulePattern,
    pub next_fire_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
struct ScheduledJob {
    id: ScheduleJobId,
    name: String,
    schedule: ScheduledJobSchedule,
    handler: JobHandler,
    next_fire_at: Option<DateTime<Utc>>,
}

impl ScheduledJob {
    fn new(
        name: impl Into<String>,
        schedule: ScheduledJobSchedule,
        handler: JobHandler,
        now: DateTime<Utc>,
    ) -> Result<Self, ScheduleError> {
        let name = name.into();
        let next_fire_at = schedule
            .initial_fire_at(now)
            .ok_or_else(|| match &schedule {
                ScheduledJobSchedule::Cron { expression, .. } => {
                    ScheduleError::NoUpcomingFireTime {
                        expression: expression.clone(),
                    }
                }
                ScheduledJobSchedule::Interval { every } => ScheduleError::NoUpcomingFireTime {
                    expression: format!("interval:{every:?}"),
                },
                ScheduledJobSchedule::Timeout { delay } => ScheduleError::NoUpcomingFireTime {
                    expression: format!("timeout:{delay:?}"),
                },
            })?;

        Ok(Self {
            id: Uuid::new_v4(),
            name,
            schedule,
            handler,
            next_fire_at: Some(next_fire_at),
        })
    }

    fn info(&self) -> ScheduledJobInfo {
        ScheduledJobInfo {
            id: self.id,
            name: self.name.clone(),
            pattern: self.schedule.pattern(),
            next_fire_at: self.next_fire_at,
        }
    }

    fn snapshot_if_due(&self, now: DateTime<Utc>) -> Option<ScheduledJobSnapshot> {
        let next_fire_at = self.next_fire_at?;
        if next_fire_at > now {
            return None;
        }

        Some(ScheduledJobSnapshot {
            id: self.id,
            name: self.name.clone(),
            schedule: self.schedule.clone(),
            handler: self.handler.clone(),
            due_at: next_fire_at,
        })
    }
}

#[derive(Clone)]
struct ScheduledJobSnapshot {
    id: ScheduleJobId,
    name: String,
    schedule: ScheduledJobSchedule,
    handler: JobHandler,
    due_at: DateTime<Utc>,
}

/// In-memory scheduler with cron, interval, and timeout support.
///
/// Jobs are registered explicitly and `tick_at` advances the due jobs against
/// a caller-supplied clock.
#[derive(Clone, Default)]
pub struct ScheduleModule {
    jobs: Arc<RwLock<HashMap<ScheduleJobId, ScheduledJob>>>,
}

impl ScheduleModule {
    /// Create an empty scheduler.
    pub fn new() -> Self {
        Self::default()
    }

    async fn register_job(
        &self,
        name: impl Into<String>,
        schedule: ScheduledJobSchedule,
        now: DateTime<Utc>,
        handler: JobHandler,
    ) -> Result<ScheduleJobId, ScheduleError> {
        let job = ScheduledJob::new(name, schedule, handler, now)?;
        let id = job.id;
        self.jobs.write().await.insert(id, job);
        Ok(id)
    }

    /// Register a schedule pattern using the current UTC time.
    pub async fn register_pattern<F, Fut>(
        &self,
        name: impl Into<String>,
        pattern: SchedulePattern,
        handler: F,
    ) -> Result<ScheduleJobId, ScheduleError>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        self.register_pattern_at(name, pattern, Utc::now(), handler)
            .await
    }

    /// Register a schedule pattern using a caller-supplied clock.
    pub async fn register_pattern_at<F, Fut>(
        &self,
        name: impl Into<String>,
        pattern: SchedulePattern,
        now: DateTime<Utc>,
        handler: F,
    ) -> Result<ScheduleJobId, ScheduleError>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        let handler: JobHandler = Arc::new(move || Box::pin(handler()));
        self.register_job(
            name,
            ScheduledJobSchedule::from_pattern(pattern)?,
            now,
            handler,
        )
        .await
    }

    /// Register a cron job using the current UTC time as the initial clock.
    pub async fn register_cron<F, Fut>(
        &self,
        name: impl Into<String>,
        expression: impl Into<String>,
        handler: F,
    ) -> Result<ScheduleJobId, ScheduleError>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        self.register_pattern_at(
            name,
            SchedulePattern::cron(expression.into()),
            Utc::now(),
            handler,
        )
        .await
    }

    /// Register a cron job using a caller-supplied clock.
    pub async fn register_cron_at<F, Fut>(
        &self,
        name: impl Into<String>,
        expression: impl Into<String>,
        now: DateTime<Utc>,
        handler: F,
    ) -> Result<ScheduleJobId, ScheduleError>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        let handler: JobHandler = Arc::new(move || Box::pin(handler()));
        self.register_job(
            name,
            ScheduledJobSchedule::from_pattern(SchedulePattern::cron(expression.into()))?,
            now,
            handler,
        )
        .await
    }

    /// Register a repeating interval job using the current UTC time.
    pub async fn register_interval<F, Fut>(
        &self,
        name: impl Into<String>,
        every: std::time::Duration,
        handler: F,
    ) -> Result<ScheduleJobId, ScheduleError>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        self.register_pattern_at(name, SchedulePattern::interval(every), Utc::now(), handler)
            .await
    }

    /// Register a repeating interval job using a caller-supplied clock.
    pub async fn register_interval_at<F, Fut>(
        &self,
        name: impl Into<String>,
        every: std::time::Duration,
        now: DateTime<Utc>,
        handler: F,
    ) -> Result<ScheduleJobId, ScheduleError>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        if every.is_zero() {
            return Err(ScheduleError::InvalidIntervalDuration {
                every_ms: every.as_millis() as u64,
            });
        }

        let handler: JobHandler = Arc::new(move || Box::pin(handler()));
        self.register_job(
            name,
            ScheduledJobSchedule::from_pattern(SchedulePattern::interval(every))?,
            now,
            handler,
        )
        .await
    }

    /// Register a one-shot timeout job using the current UTC time.
    pub async fn register_timeout<F, Fut>(
        &self,
        name: impl Into<String>,
        delay: std::time::Duration,
        handler: F,
    ) -> Result<ScheduleJobId, ScheduleError>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        self.register_pattern_at(name, SchedulePattern::timeout(delay), Utc::now(), handler)
            .await
    }

    /// Register a one-shot timeout job using a caller-supplied clock.
    pub async fn register_timeout_at<F, Fut>(
        &self,
        name: impl Into<String>,
        delay: std::time::Duration,
        now: DateTime<Utc>,
        handler: F,
    ) -> Result<ScheduleJobId, ScheduleError>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        let handler: JobHandler = Arc::new(move || Box::pin(handler()));
        self.register_job(
            name,
            ScheduledJobSchedule::from_pattern(SchedulePattern::timeout(delay))?,
            now,
            handler,
        )
        .await
    }

    /// Remove a job by id.
    pub async fn remove_job(&self, id: ScheduleJobId) -> bool {
        self.jobs.write().await.remove(&id).is_some()
    }

    /// Return the current metadata for a registered job.
    pub async fn job(&self, id: ScheduleJobId) -> Option<ScheduledJobInfo> {
        self.jobs.read().await.get(&id).map(ScheduledJob::info)
    }

    /// Return the number of registered jobs.
    pub async fn job_count(&self) -> usize {
        self.jobs.read().await.len()
    }

    /// Return a snapshot of all jobs.
    pub async fn jobs(&self) -> Vec<ScheduledJobInfo> {
        self.jobs
            .read()
            .await
            .values()
            .map(ScheduledJob::info)
            .collect()
    }

    /// Return the next fire time for one job.
    pub async fn next_fire_at(&self, id: ScheduleJobId) -> Option<DateTime<Utc>> {
        self.jobs
            .read()
            .await
            .get(&id)
            .and_then(|job| job.next_fire_at)
    }

    /// Advance the scheduler to `now` and invoke every due job.
    pub async fn tick_at(&self, now: DateTime<Utc>) -> Result<Vec<ScheduleJobId>, ScheduleError> {
        let mut fired = Vec::new();

        loop {
            let next_due = {
                let jobs = self.jobs.read().await;
                jobs.values()
                    .filter_map(|job| job.snapshot_if_due(now))
                    .min_by_key(|job| job.due_at)
            };

            let Some(snapshot) = next_due else {
                break;
            };

            let result = (snapshot.handler)().await;

            let next_fire_at = snapshot.schedule.next_after(snapshot.due_at);
            let mut jobs = self.jobs.write().await;
            if let Some(job) = jobs.get_mut(&snapshot.id) {
                job.next_fire_at = next_fire_at;
            }
            if next_fire_at.is_none() {
                jobs.remove(&snapshot.id);
            }

            match result {
                Ok(()) => fired.push(snapshot.id),
                Err(message) => {
                    return Err(ScheduleError::JobFailed {
                        job_name: snapshot.name,
                        message,
                    });
                }
            }
        }

        Ok(fired)
    }

    /// Advance the scheduler using the current UTC time.
    pub async fn tick(&self) -> Result<Vec<ScheduleJobId>, ScheduleError> {
        self.tick_at(Utc::now()).await
    }
}

#[async_trait]
impl Injectable for ScheduleModule {
    async fn build(_container: &DependencyContainer) -> Result<Self, DiError> {
        Ok(Self::new())
    }

    fn dependencies() -> Vec<TypeId> {
        Vec::new()
    }
}

fn duration_to_chrono(duration: std::time::Duration) -> Option<ChronoDuration> {
    ChronoDuration::from_std(duration).ok()
}
