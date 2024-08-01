use std::{collections::HashSet, fmt};

use chrono::{DateTime, Local, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{error::ErrorKind, SqlitePool};
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
    pub id: i64,
    pub title: String,
    pub author: String,
    pub format: ExportFormat,
    pub state: ExportState,
    pub step: ExportStep,
    pub progress: i64,
    pub created_at: NaiveDateTime,
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
            created_at as "created_at: NaiveDateTime"
        FROM Export WHERE Export.id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await?;
    Ok(export)
}

pub async fn insert_export(
    pool: &SqlitePool,
    title: &str,
    author: &str
) -> Result<i64, sqlx::Error> {
    let now = chrono::Local::now().to_rfc3339();
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

pub async fn create_export(pool: &SqlitePool, title: &str, author: &str) -> Result<i64, sqlx::Error> {
    let id = match insert_export(pool, title, author).await {
        Ok(id) => Ok(id),
        Err(sqlx::Error::Database(e)) => {
            match e.kind() {
                ErrorKind::UniqueViolation => {
                    let new_title = format!("{} ({})", title, OffsetDateTime::now_utc());
                    Ok(insert_export(pool, &new_title, author).await?)
                }
                _ => Err(sqlx::Error::Database(e)),
            }
        },
        Err(e) => Err(e),
    }?;

    Ok(id)
}

pub async fn set_chapters_for_export(
    pool: &SqlitePool,
    export_id: i64,
    chapter_ids: HashSet<i64>,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    sqlx::query!(
        r#"
        DELETE FROM ExportChapters
        WHERE export_id = ?
        "#,
        export_id
    )
    .execute(&mut *tx)
    .await?;

    for chapter_id in chapter_ids {
        sqlx::query!(
            r#"
            INSERT INTO ExportChapters (export_id, chapter_id)
            VALUES (?, ?)
            "#,
            export_id,
            chapter_id
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
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

pub async fn set_export_step(pool: &SqlitePool, id: i64, step: ExportStep) -> Result<(), AppError> {
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

pub async fn get_export_and_chapters_by_id(
    pool: &SqlitePool,
    id: i64,
) -> Result<Option<(Export, HashSet<i64>)>, AppError> {
    let export = get_export_by_id(pool, id).await?;
    let chapters: HashSet<i64> = sqlx::query!(
        r#"
        SELECT chapter_id
        FROM ExportChapters
        WHERE export_id = ?
        "#,
        id
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| row.chapter_id)
    .filter_map(|chapter_id| chapter_id)
    .collect();

    Ok(export.map(|export| (export, chapters)))
}

pub async fn get_export_list(pool: &SqlitePool) -> Result<Vec<Export>, AppError> {
    let exports = sqlx::query_as!(
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
        FROM Export
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(pool)
    .await?;
    Ok(exports)
}
