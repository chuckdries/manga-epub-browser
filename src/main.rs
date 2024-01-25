extern crate dotenv;
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::Arc,
};
use anyhow::{anyhow, Error, Result};
use askama::Template;
use axum::{
    debug_handler,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Extension, Router,
};
use axum_extra::extract::Form;
use ebook::commit_chapter_selection;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
// use suwayomi::download_chapters;
// use suwayomi::get_chapters_by_id;
use dotenv::dotenv;
use std::env;
// use tokio::sync::RwLock;
use tower_sessions::{cookie::time::Duration, Expiry, MemoryStore, Session, SessionManagerLayer};

mod ebook;
mod suwayomi;
mod util; // Declare the util module


// struct AppState {
//     config: RwLock<AppConfig>, // Use RwLock for thread-safe access
// }

// CQ clean up this error handling mess
// Make our own error that wraps `anyhow::Error`.
struct AppError(anyhow::Error);

#[derive(Template)]
#[template(path = "error.html")]
struct ErrorPageTemplate {
    message: String,
    status_code: StatusCode,
}

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
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
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

async fn not_found() -> Html<&'static str> {
    Html("<h1>404 Not found</h1><a href=\"/\">Back home</a>")
}

#[derive(Template)]
#[template(path = "search-results.html")]
struct SearchResultsTemplate {
    title: String,
    mangas: Vec<suwayomi::manga_search_by_title::MangaSearchByTitleMangasNodes>,
    api_base: String,
}

#[debug_handler]
async fn search_results(
    Query(params): Query<HashMap<String, String>>,
) -> Result<SearchResultsTemplate, AppError> {
    let title = params.get("title").unwrap().to_string();
    let res = suwayomi::search_manga_by_title(
        suwayomi::manga_search_by_title::Variables {
            title: title.clone(),
        },
    )
    .await?;
    Ok(SearchResultsTemplate {
        title: title,
        mangas: res,
        api_base: env::var("SUWAYOMI_URL")?,
    })
}

#[derive(Template)]
#[template(path = "manga-page.html")]
struct MangaPageTemplate {
    manga: suwayomi::specific_manga_by_id::SpecificMangaByIdManga,
    api_base: String,
}

#[debug_handler]
async fn manga_by_id(
    params: Path<i64>,
) -> Result<MangaPageTemplate, AppError> {
    let manga = suwayomi::get_manga_by_id(params.0).await?;
    Ok(MangaPageTemplate {
        manga,
        api_base: env::var("SUWAYOMI_URL")?,
    })
}

#[derive(Template)]
#[template(path = "chapter-select.html")]
struct ChapterSelectTemplate {
    manga_id: i64,
    title: String,
    items: Vec<suwayomi::specific_manga_chapters::SpecificMangaChaptersMangaChaptersNodes>,
    selected: HashSet<i64>,
    limit: usize,
    offset: usize,
}

#[debug_handler]
async fn get_chapters_by_manga_id(
    params: Path<i64>,
) -> Result<ChapterSelectTemplate, AppError> {
    let (title, chapters) = suwayomi::get_chapters_by_id(params.0).await?;
    let limit = 20;
    let offset = 0;
    // CQ: TODO avoid this copy
    let items = chapters[offset..limit].to_vec();
    Ok(ChapterSelectTemplate {
        manga_id: params.0,
        title,
        items,
        limit,
        offset,
        selected: HashSet::new(),
    })
}

#[derive(Deserialize)]
struct ChapterSelectInput {
    #[serde(default)]
    selected_items: Vec<String>,
    page_control: Option<String>,
}

const SESSION_OFFSET_KEY: &str = "offset";
#[derive(Default, Deserialize, Serialize)]
struct SessionOffset(usize);
const SESSION_SELECTED_CHAPTERS_KEY: &str = "selected_chapters";
#[derive(Default, Deserialize, Serialize)]
struct SessionSelectedChapters(HashMap<usize, HashSet<i64>>);

// Define an enum to hold either a Template or a String
enum PostChapterResponse {
    TemplateResponse(ChapterSelectTemplate),
    RedirectResponse(Redirect),
}

// Implement IntoResponse for MyResponse
impl IntoResponse for PostChapterResponse {
    fn into_response(self) -> Response {
        match self {
            PostChapterResponse::TemplateResponse(template) => template.into_response(),
            PostChapterResponse::RedirectResponse(r) => r.into_response(),
        }
    }
}

fn concat_chapter_ids(
    session_selected: HashMap<usize, HashSet<i64>>,
    current_selection: HashSet<i64>,
    current_page: usize,
) -> HashSet<i64> {
    let mut all_selected: HashSet<i64> = HashSet::new();
    for page in session_selected.keys() {
        if *page != current_page {
            let page_selected = session_selected.get(page).unwrap();
            all_selected.extend(page_selected)
        }
    }
    all_selected.extend(current_selection);
    all_selected
}

// CQ: plan
// 1. Create Book in database with mangaid and selected chapters
// 2. prompt user for book title (default: Title + chapter range)
// 3. compile book
//    1. instruct suwayomi to download chapters from source
//    2. fetch chapter pages from suwayomi
//    3. assemble epub
// 4. offer for download
// book will include status field
// homepage will list books and their statuses
// download_chapters(all_chapters).await?;

#[debug_handler]
async fn post_chapters_by_manga_id(
    Extension(pool): Extension<SqlitePool>,
    params: Path<i64>,
    session: Session,
    Form(data): Form<ChapterSelectInput>,
) -> Result<PostChapterResponse, AppError> {
    let limit = 20;
    let offset: SessionOffset = session
        .get(SESSION_OFFSET_KEY)
        .await
        .unwrap()
        .unwrap_or_default();

    let prev_page_offset = offset.0;

    let mut session_selected_chapters = session
        .get::<SessionSelectedChapters>(SESSION_SELECTED_CHAPTERS_KEY)
        .await
        .unwrap()
        .unwrap_or_default()
        .0;

    let new_chapters_selected = data
        .selected_items
        .iter()
        .map(|s| {
            s.parse::<i64>()
                .expect("bad ID in form body selected_items")
        })
        .collect();

    let new_current_page = match data.page_control {
        Some(s) if s == "prev" && offset.0 >= limit => prev_page_offset - limit,
        Some(s) if s == "next" => prev_page_offset + limit,
        Some(_) => prev_page_offset,
        None => {
            let all_chapters = concat_chapter_ids(
                session_selected_chapters,
                new_chapters_selected,
                prev_page_offset,
            );

            let book_id = commit_chapter_selection(pool, all_chapters, params.0).await?;

            // redirect to book configuration page
            return Ok(PostChapterResponse::RedirectResponse(Redirect::to(
                &format!("/configure-book/{}", book_id),
            )));
        }
    };

    let previously_selected_chapters = match session_selected_chapters.get(&new_current_page) {
        // CQ: TODO avoid this copy
        Some(s) => s.clone(),
        None => HashSet::new(),
    };

    session_selected_chapters.insert(prev_page_offset, new_chapters_selected);

    session
        .insert(
            SESSION_SELECTED_CHAPTERS_KEY,
            SessionSelectedChapters(session_selected_chapters),
        )
        .await?;
    session.insert(SESSION_OFFSET_KEY, new_current_page).await?;

    let end = new_current_page + limit;

    let (title, chapters) = suwayomi::get_chapters_by_id(params.0).await?;
    // CQ: TODO avoid this copy
    let items = chapters[new_current_page..end].to_vec();

    Ok(PostChapterResponse::TemplateResponse(
        ChapterSelectTemplate {
            manga_id: params.0,
            title,
            items,
            limit,
            offset: new_current_page,
            selected: previously_selected_chapters,
        },
    ))
}

#[derive(Template)]
#[template(path = "home.html")]
struct HomeTemplate {}

#[debug_handler]
async fn home() -> Result<HomeTemplate, AppError> {
    Ok(HomeTemplate {})
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    // Create a connection pool
    let pool =
        SqlitePool::connect(&env::var("DATABASE_URL").expect("DATABASE_URL is a required field"))
            .await
            .expect("Failed to create pool.");

    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_expiry(Expiry::OnInactivity(Duration::days(14)));

    // build our application with a single route
    let app = Router::new()
        .route("/", get(home))
        .route("/search", get(search_results))
        .route("/manga/:id", get(manga_by_id))
        .route("/manga/:id/chapters", get(get_chapters_by_manga_id))
        .route("/manga/:id/chapters", post(post_chapters_by_manga_id))
        .fallback(not_found)
        .layer(Extension(pool))
        .layer(session_layer);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
