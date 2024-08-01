use std::sync::Arc;

use askama::Template;
use axum::{debug_handler, extract::Path, Extension};
use eyre::eyre;
use sqlx::SqlitePool;

use crate::{
    ebook::{get_book_with_chapters_by_id, Book},
    services::book_compiler::assemble_epub,
    suwayomi::{chapters_by_ids::ChaptersByIdsChaptersNodes, get_chapters_by_ids},
    AppError,
};

#[derive(Template)]
#[template(path = "export-details.html")]
pub struct BookDetailsTemplate {
    book: Book,
    chapters: Vec<ChaptersByIdsChaptersNodes>,
}

// TODO task log
// TODO status from tasks table
#[debug_handler]
pub async fn view_book_details(
    Extension(pool): Extension<Arc<SqlitePool>>,
    Path(id): Path<i64>,
) -> Result<BookDetailsTemplate, AppError> {
    let book = match get_book_with_chapters_by_id(&pool, id).await? {
        Some(book) => book,
        None => return Err(eyre!("Book not found").into()),
    };
    let chapters = match get_chapters_by_ids(&book.chapters).await? {
        Some(chapters) => chapters.nodes,
        None => return Err(eyre!("Chapters not found").into()),
    };
    dbg!(&chapters);
    let template = BookDetailsTemplate {
        book: book.book,
        chapters,
    };

    Ok(template)
}

pub async fn post_assemble_epub(
    Extension(pool): Extension<Arc<SqlitePool>>,
    Path(id): Path<i64>,
) -> Result<(), AppError> {
    let book_with_chapters = match get_book_with_chapters_by_id(&pool, id).await? {
        Some(book_with_chapters) => book_with_chapters,
        None => return Err(eyre!("Book not found").into()),
    };
    assemble_epub(book_with_chapters.book, &book_with_chapters.chapters).await?;
    Ok(())
}
