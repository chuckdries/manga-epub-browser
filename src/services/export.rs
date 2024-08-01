use std::fmt;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use time::OffsetDateTime;

use crate::AppError;

#[derive(Debug, Serialize, Deserialize, sqlx::Type, PartialEq, Clone, Copy)]
#[sqlx(rename_all = "snake_case")]
pub enum ExportStep {
    Begin,
    DownloadingFromSource,
    FetchingFromSuwayomi,
    AssemblingFile,
    Complete,
}

impl std::fmt::Display for ExportStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportStep::Begin => write!(f, "Draft"),
            ExportStep::DownloadingFromSource => write!(f, "Downloading from source"),
            ExportStep::FetchingFromSuwayomi => write!(f, "Fetching from Suwayomi"),
            ExportStep::AssemblingFile => write!(f, "Assembling file"),
            ExportStep::Complete => write!(f, "Complete"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(rename_all = "snake_case")]
pub enum ExportState {
    Draft,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(rename_all = "snake_case")]
pub enum ExportFormat {
    Epub,
    Cbz,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Export {
    id: i64,
    title: String,
    author: String,
    format: ExportFormat,
    state: ExportState,
    step: ExportStep,
    progress: i64,
    created_at: OffsetDateTime,
}

pub async fn get_export_by_id(pool: &SqlitePool, id: i64) -> Result<Option<Export>, AppError> {
    let export = sqlx::query_as!(
        Export,
        r#"
         SELECT 
            id,
            title,
            author,
            format as "format: ExportFormat",
            state as "state: ExportState",
            step as "step: ExportStep",
            progress,
            created_at as "created_at: OffsetDateTime"
        FROM Export WHERE Export.id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(export)
}

pub async fn create_export(pool: &SqlitePool, title: &str, author: &str) -> Result<i64, AppError> {
    let now = OffsetDateTime::now_utc();
    let id = sqlx::query!(
        r#"
        INSERT INTO Export (title, author, format, state, step, progress, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        RETURNING id
        "#,
        title,
        author,
        ExportFormat::Epub,
        ExportState::Draft,
        ExportStep::Begin,
        0,
        now
    )
    .fetch_one(pool)
    .await?
    .id;
    Ok(id)
}

pub async fn set_export_config(
    pool: &SqlitePool,
    id: i64,
    title: &str,
    author: &str,
    format: ExportFormat,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE Export
        SET title = ?, author = ?, format = ?
        WHERE id = ?
        "#,
        title,
        author,
        format,
        id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_export_state(
    pool: &SqlitePool,
    id: i64,
    state: ExportState,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE Export
        SET state = ?
        WHERE id = ?
        "#,
        state,
        id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_export_step(
    pool: &SqlitePool,
    id: i64,
    step: ExportStep,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE Export
        SET step = ?
        WHERE id = ?
        "#,
        step,
        id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_export_progress(
    pool: &SqlitePool,
    id: i64,
    progress: i64,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE Export
        SET progress = ?
        WHERE id = ?
        "#,
        progress,
        id
    )
    .execute(pool)
    .await?;
    Ok(())
}
