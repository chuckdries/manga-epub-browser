use axum::{routing::get, Router};

mod books_list;

pub fn get_routes() -> axum::Router {
    Router::new().route("/", get(books_list::view_books_list))
}
