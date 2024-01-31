use std::collections::HashSet;

use anyhow::anyhow;
use askama::Result;
use sqlx::{Acquire, SqlitePool};

use crate::AppError;

pub async fn commit_chapter_selection(
    pool: SqlitePool,
    chapters: HashSet<i64>,
    manga_id: i64,
    default_title: &str,
    default_author: &str,
) -> Result<i64, AppError> {
    let id = sqlx::query!(
        r#"
        INSERT INTO Books ( manga_id, title, author )
        VALUES ( ?1, ?2, ?3 )
        "#,
        manga_id,
        default_title,
        default_author
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
    pub title: String,
    pub author: String,
    pub chapters: HashSet<i64>,
}

pub async fn get_book_with_chapters_by_id(
    pool: SqlitePool,
    id: i64,
) -> Result<Option<Book>, AppError> {
    let book_chapters = sqlx::query!(
        r#"
    SELECT Books.manga_id, Books.title as "title?: String", Books.author as "author?: String", Books.status, BookChapters.chapter_id 
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
    let mut chapters: HashSet<i64> = HashSet::new();
    let mut manga_id: i64 = 0;
    let mut title: String = "".to_string();
    let mut author: String = "".to_string();
    book_chapters.iter().for_each(|chapter| {
        manga_id = chapter.manga_id.expect("Book missing manga_id");
        title = chapter.title.expect("Book missing title");
        author = chapter.author.expect("Book missing author");
        chapters.insert(chapter.chapter_id.expect("BookChapter missing chapter_id"));
    });

    dbg!(&chapters);

    Ok(Some(Book {
        id,
        manga_id,
        title,
        author,
        chapters,
    }))
}
