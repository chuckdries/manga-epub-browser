use axum::{routing::get, Router};

pub mod configure;
mod details;
mod download;

pub fn get_routes() -> axum::Router {
    Router::new()
        .route("/:id", get(details::view_book_details))
        .route(
            "/:id/configure",
            get(configure::view_configure_book).post(configure::post_configure_export),
        )
        .route("/:id/download", get(download::serve_export))
}
