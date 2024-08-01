use std::sync::Arc;

use askama::Template;
use axum::Extension;
use sqlx::SqlitePool;

use crate::{
    ebook::{get_book_table, Book},
    AppError,
};

#[derive(Template)]
#[template(path = "books-list.html")]
pub struct BooksList {
    books: Vec<Book>,
}

#[axum::debug_handler]
pub async fn view_books_list(
    Extension(pool): Extension<Arc<SqlitePool>>,
) -> Result<BooksList, AppError> {
    let books = get_book_table(&pool).await?;
    Ok(BooksList { books })
}
