use axum::{routing::get, Router};

mod chapter_select;
mod manga_select;

#[axum::debug_handler]
async fn redirect_to_manga_select() -> axum::response::Redirect {
    axum::response::Redirect::to("/book/new/select-manga")
}

pub fn get_routes() -> axum::Router {
    Router::new()
        .route("/", get(redirect_to_manga_select))
        .route("/select-manga", get(manga_select::view_manga_select))
        .route(
            "/select-chapters",
            get(chapter_select::view_chapter_select).post(chapter_select::post_chapter_select),
        )
}
