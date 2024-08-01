use serde::Serialize;
use sqlx::prelude::FromRow;



#[derive(Debug, Serialize)]
pub enum Status {
    Pending = 0,
    Success = 1,
    Failed = 2,
}

impl From<i64> for Status {
    fn from(value: i64) -> Self {
        match value {
            0 => Status::Pending,
            1 => Status::Success,
            2 => Status::Failed,
            _ => Status::Pending,
        }
    }
}

#[derive(Debug, Serialize, FromRow)]
struct ExportLog {
    id: i64,
    status: Status,
    book_id: i64,
    file_path: String,
    date: String,
}

pub async fn get_export_logs(pool: &sqlx::SqlitePool) -> Result<Vec<ExportLog>, sqlx::Error> {
    let logs = sqlx::query_as!(
        ExportLog,
        r#"
        SELECT id, status, book_id, file_path, date
        FROM ExportLogs
        ORDER BY date DESC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(logs)
}