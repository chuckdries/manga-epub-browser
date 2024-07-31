use std::{collections::HashSet, fmt};

use askama::Result;
use serde::Serialize;
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

#[derive(sqlx::FromRow, Serialize)]
pub struct SqlBook {
    pub id: i64,
    pub manga_id: i64,
    pub title: String,
    pub author: String,
    pub status: i64,
}

#[derive(Debug)]
pub struct Book {
    pub id: i64,
    pub manga_id: i64,
    pub title: String,
    pub author: String,
    pub status: BookStatus,
}

pub async fn get_book_by_id(pool: &SqlitePool, id: i64) -> Result<Option<Book>, AppError> {
    let book = sqlx::query_as!(SqlBook, r#" SELECT * FROM Books WHERE Books.id = ?"#, id)
        .fetch_one(pool)
        .await?;
    Ok(Some(Book {
        id: book.id,
        manga_id: book.manga_id,
        title: book.title,
        author: book.author,
        status: BookStatus::from_u8(book.status as u8),
    }))
}

pub async fn get_book_table(pool: &SqlitePool) -> Result<Vec<Book>, AppError> {
    let books = sqlx::query_as!(SqlBook, r#" SELECT * FROM Books"#)
        .map(|b| Book {
            id: b.id,
            manga_id: b.manga_id,
            title: b.title,
            author: b.author,
            status: BookStatus::from_u8(b.status as u8),
        })
        .fetch_all(pool)
        .await?;
    Ok(books)
}

pub struct BookWithChapterSet {
    pub book: Book,
    pub chapters: HashSet<i64>,
}

pub async fn get_book_with_chapters_by_id(
    pool: &SqlitePool,
    id: i64,
) -> Result<Option<BookWithChapterSet>, AppError> {
    let book_chapters = sqlx::query!(
        r#"
    SELECT Books.manga_id, Books.title, Books.author, Books.status, BookChapters.chapter_id 
    FROM Books 
    LEFT JOIN BookChapters WHERE BookChapters.book_id = Books.id 
    AND Books.id = ?"#,
        id
    )
    .fetch_all(pool)
    .await?;

    if book_chapters.len() == 0 {
        return Ok(None);
    }
    let mut chapters: HashSet<i64> = HashSet::new();
    let mut book: Option<Book> = None;
    book_chapters.iter().for_each(|chapter| {
        if book.is_none() {
            book = Some(Book {
                id,
                manga_id: chapter.manga_id,
                title: chapter.title.to_owned(),
                author: chapter.author.to_owned(),
                status: BookStatus::from_u8(chapter.status as u8)
            });
        }
        chapters.insert(chapter.chapter_id.expect("BookChapter missing chapter_id"));
    });

    Ok(Some(BookWithChapterSet {
        book: book.expect("Book missing params"),
        chapters,
    }))
}

#[derive(Debug, PartialEq)]
pub enum BookStatus {
    DRAFT = 1,
    DOWNLOADING = 2,
    ASSEMBLING = 3,
    DONE = 4,
    ERROR = 5,
}

impl BookStatus {
    pub fn as_u8(&self) -> u8 {
        match *self {
            BookStatus::DRAFT => 1,
            BookStatus::DOWNLOADING => 2,
            BookStatus::ASSEMBLING => 3,
            BookStatus::DONE => 4,
            BookStatus::ERROR => 5,
        }
    }
    pub fn from_u8(status: u8) -> BookStatus {
        match status {
            1 => BookStatus::DRAFT,
            2 => BookStatus::DOWNLOADING,
            3 => BookStatus::ASSEMBLING,
            4 => BookStatus::DONE,
            _ => BookStatus::ERROR,
        }
    }
}

impl fmt::Display for BookStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BookStatus::DRAFT => write!(f, "Draft"),
            BookStatus::DOWNLOADING => write!(f, "Downloading"),
            BookStatus::ASSEMBLING => write!(f, "Assembling"),
            BookStatus::DONE => write!(f, "Done"),
            BookStatus::ERROR => write!(f, "Error"),
        }
    }
}

pub async fn update_book_status(
    pool: &SqlitePool,
    id: i64,
    _status: BookStatus,
) -> Result<(), AppError> {
    let status = _status as u8;
    sqlx::query!(
        r#"
        UPDATE Books
        SET status = ?
        WHERE id = ?
        "#,
        status,
        id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_book_details(
    pool: &SqlitePool,
    id: i64,
    title: &str,
    author: &str,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE Books
        SET title = ?, author = ?
        WHERE id = ?
        "#,
        title,
        author,
        id
    )
    .execute(pool)
    .await?;
    Ok(())
}
