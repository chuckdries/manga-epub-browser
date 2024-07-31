use anyhow::anyhow;
use askama::Template;
use askama_axum::IntoResponse;
use axum::{
    extract::Path,
    response::{Redirect, Response},
    Extension,
};
use axum_extra::extract::Form;
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{
    ebook::{self, get_book_with_chapters_by_id, update_book_details},
    services::book_compiler::begin_compile_book,
    suwayomi::{
        self,
        chapters_by_ids::{ChaptersByIdsChapters, ChaptersByIdsChaptersNodes},
    },
    AppError,
};

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
) -> Result<Response, AppError> {
    let book_id = params.0;
    let book_details = match ebook::get_book_with_chapters_by_id(&pool, book_id).await? {
        Some(book_details) => Ok(book_details),
        None => Err(AppError(anyhow!("Book not found"))),
    }?;
    if book_details.book.status != ebook::BookStatus::Draft {
        return Err(AppError(anyhow!("Book is not in draft status")));
    }
    let chapter_details = match suwayomi::get_chapters_by_ids(&book_details.chapters).await? {
        Some(chapter_details) => Ok(chapter_details),
        None => Err(AppError(anyhow!("Chapters not found"))),
    }?;
    Ok(BookConfigure {
        book: book_details.book,
        chapters: chapter_details.nodes,
    }
    .into_response())
}

#[derive(Deserialize)]
pub struct ConfigureBookInput {
    title: String,
    author: String,
    action: String,
}

pub async fn post_configure_book(
    Extension(pool): Extension<SqlitePool>,
    Path(id): Path<i64>,
    Form(data): Form<ConfigureBookInput>,
) -> Result<Response, AppError> {
    update_book_details(&pool, id, &data.title, &data.author).await?;
    // do this to render template
    // view_configure_book(Extension(pool), Path(id)).await
    if data.action == "save" {
        return Ok(Redirect::to("/books").into_response());
    }
    begin_compile_book(pool, id).await?;
    // Ok(Redirect::to(&format!("/book/{}/status", id)).into_response())
    Ok(Redirect::to("/books").into_response())
}
