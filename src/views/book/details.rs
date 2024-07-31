use anyhow::anyhow;
use askama::Template;
use axum::{debug_handler, extract::Path, Extension};
use sqlx::SqlitePool;

use crate::{
    ebook::{get_book_with_chapters_by_id, Book},
    suwayomi::{
        chapters_by_ids::ChaptersByIdsChaptersNodes,
        get_chapters_by_ids,
    },
    AppError,
};

#[derive(Template)]
#[template(path = "book-details.html")]
pub struct BookDetailsTemplate {
    book: Book,
    chapters: Vec<ChaptersByIdsChaptersNodes>,
}

#[debug_handler]
pub async fn view_book_details(
    Extension(pool): Extension<SqlitePool>,
    Path(id): Path<i64>,
) -> Result<BookDetailsTemplate, AppError> {
    let book = match get_book_with_chapters_by_id(&pool, id).await? {
        Some(book) => book,
        None => return Err(anyhow!("Book not found").into()),
    };
    let chapters = match get_chapters_by_ids(&book.chapters).await? {
        Some(chapters) => chapters.nodes,
        None => return Err(anyhow!("Chapters not found").into()),
    };
    dbg!(&chapters);
    let template = BookDetailsTemplate {
        book: book.book,
        chapters,
    };

    Ok(template)
}
