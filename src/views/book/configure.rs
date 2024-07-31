use anyhow::anyhow;
use askama::Template;
use askama_axum::IntoResponse;
use axum::{extract::Path, Extension};
use sqlx::SqlitePool;

use crate::{ebook, suwayomi::{self, chapters_by_ids::{ChaptersByIdsChapters, ChaptersByIdsChaptersNodes}}, AppError};


#[derive(Template)]
#[template(path = "book-configure.html")]
pub struct BookConfigure {
    book: ebook::Book,
    chapters: Vec<ChaptersByIdsChaptersNodes>,
}

#[axum::debug_handler]
pub async fn view_configure_book(
    Extension(pool): Extension<SqlitePool>,
    params: Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let book_id = params.0;
    let book_details = match ebook::get_book_with_chapters_by_id(&pool, book_id).await? {
        Some(book_details) => Ok(book_details),
        None => Err(AppError(anyhow!("Book not found"))),
    }?;
    if book_details.book.status != ebook::BookStatus::DRAFT {
        return Err(AppError(anyhow!("Book is not in draft status")));
    }
    let chapter_details = match suwayomi::get_chapters_by_ids(&book_details.chapters).await? {
        Some(chapter_details) => Ok(chapter_details),
        None => Err(AppError(anyhow!("Chapters not found"))),
    }?;
    Ok(BookConfigure {
        book: book_details.book,
        chapters: chapter_details.nodes,
    })
}
