use axum::{routing::get, Router};

mod chapter_select;
mod manga_select;

pub fn get_routes() -> axum::Router {
    Router::new()
        .route("/select-manga", get(manga_select::view_manga_select))
        .route(
            "/select-chapters",
            get(chapter_select::view_chapter_select).post(chapter_select::post_chapter_select),
        )

}
