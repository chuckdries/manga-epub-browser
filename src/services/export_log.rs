use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::AppError;

use super::export::ExportStep;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ExportLog {
    id: i64,
    export_id: i64,
    step: ExportStep,
    message: String,
    timestamp: DateTime<Local>,
}

impl std::fmt::Display for ExportLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] export {}, step {}, message {}",
            self.timestamp, self.export_id, self.step, self.message
        )
    }
}

pub async fn log_export_step(
    pool: &SqlitePool,
    export_id: i64,
    step: ExportStep,
    message: &str,
) -> Result<(), AppError> {
    let now = chrono::Local::now().to_rfc3339();
    println!(
        "[{}] export {}, step {}, message {}",
        now, export_id, step, message
    );
    sqlx::query!(
        "INSERT INTO ExportLogs (export_id, step, message, timestamp) VALUES (?, ?, ?, ?)",
        export_id,
        step,
        message,
        now,
    )
    .execute(pool)
    .await?;

    Ok(())
}
