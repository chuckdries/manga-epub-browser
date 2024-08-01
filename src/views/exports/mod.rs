use axum::{routing::get, Router};

mod export_list;

pub fn get_routes() -> axum::Router {
    Router::new().route("/", get(export_list::view_export_list))
}
