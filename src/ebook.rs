use std::collections::HashSet;

use anyhow::anyhow;
use askama::Result;
use sqlx::{Acquire, SqlitePool};

use crate::AppError;

pub async fn commit_chapter_selection(
    pool: SqlitePool,
    chapters: HashSet<i64>,
    manga_id: i64,
) -> Result<i64, AppError> {
    let id = sqlx::query!(
        r#"
        INSERT INTO Books ( manga_id )
        VALUES ( ?1 )
        "#,
        manga_id
    )
    .execute(&pool)
    .await?
    .last_insert_rowid();

    let mut tx = pool.begin().await?;

    for chapter in chapters.iter() {
        sqlx::query!(
            "INSERT INTO BookChapters (book_id, chapter_id) VALUES (?, ?)",
            id,
            chapter,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(id)
}

pub struct Book {
    pub id: i64,
    pub manga_id: i64,
    pub chapters: HashSet<i64>,
}

pub async fn get_book_with_chapters_by_id(
    pool: SqlitePool,
    id: i64,
) -> Result<Option<Book>, AppError> {
    let book_chapters = sqlx::query!(
        r#"
    SELECT Books.manga_id, BookChapters.chapter_id 
    FROM Books 
    LEFT JOIN BookChapters WHERE BookChapters.book_id = Books.id 
    AND Books.id = ?"#,
        id
    )
    .fetch_all(&pool)
    .await?;

    if book_chapters.len() == 0 {
        return Ok(None);
    }
    dbg!(&book_chapters);
    let mut chapters: HashSet<i64> = HashSet::new();
    let mut manga_id: i64 = 0;
    book_chapters.iter().for_each(|chapter| {
        manga_id = chapter.manga_id.expect("Book missing manga_id");
        chapters.insert(chapter.chapter_id.expect("BookChapter missing chapter_id"));
    });

    Ok(Some(Book {
        id,
        manga_id,
        chapters,
    }))
}
