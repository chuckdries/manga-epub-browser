use std::collections::HashSet;

use askama::Result;
use sqlx::SqlitePool;

use crate::AppError;

pub async fn commit_chapter_selection(pool: SqlitePool, chapters: HashSet<i64>, manga_id: i64) -> Result<i64, AppError> {
  let id = sqlx::query!(
    r#"
INSERT INTO Books ( title, manga_id )
VALUES ( ?1, ?2 )
    "#,
    title,
    manga_id
)
.execute(&pool)
.await?
.last_insert_rowid();
  Ok(0)
}