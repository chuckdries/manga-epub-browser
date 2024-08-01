extern crate dotenv;
use anyhow::Result;
use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
    Extension, Router,
};
use serde::{Deserialize, Serialize};
use services::book_compiler::resume_interrupted_tasks;
use sqlx::SqlitePool;
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    sync::Arc,
};
// use suwayomi::download_chapters;
// use suwayomi::get_chapters_by_id;
use dotenv::dotenv;
use std::env;
use suwayomi::{get_all_sources_by_lang, get_library};
// use tokio::sync::RwLock;
use tower_http::services::ServeDir;
use tower_sessions::{cookie::time::Duration, Expiry, MemoryStore, SessionManagerLayer};

use local_ip_address::local_ip;

mod ebook;
mod services;
mod suwayomi;
mod util; // Declare the util module
mod views;

extern crate pretty_env_logger;
// #[macro_use]
extern crate log;

// struct AppState {
//     config: RwLock<AppConfig>, // Use RwLock for thread-safe access
// }

// CQ clean up this error handling mess
// Make our own error that wraps `anyhow::Error`.
#[derive(Debug)]
struct AppError(eyre::Report);

type AppResponse = Result<Html<String>, AppError>;

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

const SESSION_OFFSET_KEY: &str = "offset";
#[derive(Default, Deserialize, Serialize)]
struct SessionOffset(usize);
const SESSION_SELECTED_CHAPTERS_KEY: &str = "selected_chapters";
#[derive(Default, Deserialize, Serialize)]
struct SessionSelectedChapters(HashMap<usize, HashSet<i64>>);

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate {
    sources: Vec<suwayomi::all_sources_by_language::AllSourcesByLanguageSourcesNodes>,
    library: Vec<suwayomi::get_library::GetLibraryMangasNodes>,
    // books: Vec<SqlBook>,
}
// Extension(pool): Extension<Arc<SqlitePool>>
async fn home() -> Result<HomeTemplate, AppError> {
    let sources = get_all_sources_by_lang(suwayomi::all_sources_by_language::Variables {
        lang: "en".to_string(),
    })
    .await
    .expect("configure sources and language before using search");

    let library = get_library().await?;
    // let books = get_book_table(&pool).await?;
    Ok(HomeTemplate { sources, library })
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    pretty_env_logger::init();

    // Create a connection pool
    let pool =
        SqlitePool::connect(&env::var("DATABASE_URL").expect("DATABASE_URL is a required field"))
            .await
            .expect("Failed to create pool.");

    println!("running migrations");

    match sqlx::migrate!().run(&pool).await {
        Ok(()) => println!("migrations succeeded"),
        Err(msg) => panic!("{}", msg),
    };

    let pool_clone = Arc::new(pool);

    resume_interrupted_tasks(pool_clone.clone()).await.unwrap();

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
        .fallback(not_found)
        .layer(Extension(pool_clone.clone()))
        .layer(session_layer);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("listening on http://{}:3000", local_ip().unwrap());
    axum::serve(listener, app).await.unwrap();
}
