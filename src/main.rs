extern crate dotenv;
use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Extension, Router,
};
use dotenv::dotenv;
use models::export::get_export_base_dir;
use services::exporter::resume_interrupted_exports;
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::{env, fs, path::Path};
use std::{fmt::Debug, str::FromStr, sync::Arc};
use tower_http::services::ServeDir;
use tower_sessions::{cookie::time::Duration, Expiry, MemoryStore, SessionManagerLayer};

use local_ip_address::local_ip;

mod models;
mod services;
mod suwayomi;
mod util;
mod views;

extern crate log;
extern crate pretty_env_logger;

// struct AppState {
//     config: RwLock<AppConfig>, // Use RwLock for thread-safe access
// }

// CQ clean up this error handling mess
// Make our own error that wraps `anyhow::Error`.
#[derive(Debug)]
struct AppError(eyre::Report);

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorPageTemplate {
    message: String,
    status_code: StatusCode,
}

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        log::error!("{:?}", self.0);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorPageTemplate {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                message: self.0.to_string(),
            },
        )
            .into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<eyre::Report>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

// impl From<eyre::Report> for AppError {
//     fn from(err: eyre::Report) -> Self {
//         Self(eyre!(err.to_string()))
//     }
// }

async fn not_found() -> impl IntoResponse {
    Html("<h1>404 Not found</h1><a href=\"/\">Back home</a>")
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    pretty_env_logger::init();

    let db_url = env::var("DATABASE_URL").expect("DATABASE_URL in .env is required");
    dbg!(&db_url);

    let connection_settings =
        SqliteConnectOptions::from_str(&db_url)
            .expect("Invalid DATABASE_URL")
            .create_if_missing(true);

    let db_base_path = Path::new(connection_settings.get_filename()).parent().unwrap();
    fs::create_dir_all(db_base_path).unwrap();
    let pool = SqlitePool::connect_with(connection_settings)
        .await
        .expect("Failed to create pool.");

    println!("running migrations");

    match sqlx::migrate!().run(&pool).await {
        Ok(()) => println!("migrations succeeded"),
        Err(msg) => panic!("{}", msg),
    };

    let pool_clone = Arc::new(pool);

    // resume_interrupted_tasks(pool_clone.clone()).await.unwrap();
    resume_interrupted_exports(pool_clone.clone())
        .await
        .unwrap();

    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_expiry(Expiry::OnInactivity(Duration::days(14)));

    let app = Router::new()
        .route("/", get(Redirect::to("/exports")))
        .nest("/export/new", views::export_new::get_routes())
        .nest("/export", views::export::get_routes())
        .nest("/exports", views::exports::get_routes())
        .nest_service("/public", ServeDir::new("public"))
        .nest_service("/download", ServeDir::new(get_export_base_dir()))
        .fallback(not_found)
        .layer(Extension(pool_clone.clone()))
        .layer(session_layer);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("listening on http://{}:3000", local_ip().unwrap());
    axum::serve(listener, app).await.unwrap();
}
