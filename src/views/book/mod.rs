use axum::{routing::{get, post}, Router};

pub mod configure;
mod details;

pub fn get_routes() -> axum::Router {
    Router::new()
        .route("/:id", get(details::view_book_details))
        .route(
            "/:id/configure",
            get(configure::view_configure_book).post(configure::post_configure_book),
        )
        // WIP manually call assemble for now
        .route(
            "/:id/assemble",
            post(details::post_assemble_epub),
        )
}
