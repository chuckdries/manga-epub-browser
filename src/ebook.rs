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
        INSERT INTO Books ( manga_id, title, author, status )
        VALUES ( ?1, ?2, ?3, ?4 )
        "#,
        manga_id,
        default_title,
        default_author,
        1
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

#[derive(sqlx::FromRow)]
pub struct Book {
    pub id: i64,
    pub manga_id: i64,
    pub title: String,
    pub author: String,
    pub status: i64,
}

pub async fn get_book_by_id(pool: SqlitePool, id: i64) -> Result<Option<Book>, AppError> {
    let book: Book = sqlx::query_as(
        r#"
        SELECT id, manga_id, title, author, status 
        FROM Books WHERE Books.id = ?"#,
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;
    Ok(Some(book))
}

pub struct BookWithChapters {
    pub book: Book,
    pub chapters: HashSet<i64>,
}

pub async fn get_book_with_chapters_by_id(
    pool: SqlitePool,
    id: i64,
) -> Result<Option<BookWithChapters>, AppError> {
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
    let mut book: Option<Book> = None;
    book_chapters.iter().for_each(|chapter| {
        if book.is_none() {
            if chapter.title.is_some() && chapter.author.is_some() && chapter.manga_id.is_some() {
                book = Some(Book {
                    id,
                    manga_id: chapter.manga_id.unwrap(),
                    title: chapter.title.to_owned().unwrap(),
                    author: chapter.author.to_owned().unwrap(),
                    status: chapter.status.to_owned(),
                });
            }
        }
        chapters.insert(chapter.chapter_id.expect("BookChapter missing chapter_id"));
    });

    dbg!(&chapters);

    Ok(Some(BookWithChapters {
        book: book.expect("Book missing params"),
        chapters,
    }))
}
