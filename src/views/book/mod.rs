use axum::{routing::get, Router};

pub mod configure;

pub fn get_routes() -> axum::Router {
    Router::new()
        .route("/:id/configure", get(configure::view_configure_book))
}
