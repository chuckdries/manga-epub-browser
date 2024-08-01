use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use time::OffsetDateTime;

use crate::AppError;

use super::book_compiler::CompileTaskStep;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaskLog {
    id: i64,
    task_id: i64,
    step: CompileTaskStep,
    message: String,
    timestamp: OffsetDateTime,
}

pub async fn log_task_step(
    pool: &SqlitePool,
    task_id: i64,
    step: CompileTaskStep,
    message: &str,
) -> Result<(), AppError> {
    let now = OffsetDateTime::now_utc();
    println!("[{}] task {}, step {}, message {}", now, task_id, step, message);
    sqlx::query!(
        "INSERT INTO TaskLogs (task_id, step, message, timestamp) VALUES (?, ?, ?, ?)",
        task_id,
        step,
        message,
        now,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_task_log_for_task(
    pool: &SqlitePool,
    task_id: i64,
) -> Result<Vec<TaskLog>, sqlx::Error> {
    let logs = sqlx::query_as!(
        TaskLog,
        r#"SELECT
            id,
            task_id,
            step as "step: CompileTaskStep",
            message,
            timestamp as "timestamp: OffsetDateTime"
            FROM TaskLogs WHERE task_id = ? ORDER BY datetime(timestamp) ASC"#,
        task_id
    )
    .fetch_all(pool)
    .await?;

    Ok(logs)
}
