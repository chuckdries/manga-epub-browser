use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::{anyhow, Error, Result};
use askama::Template;
use axum::{
    debug_handler,
    extract::{Form, Path, Query},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Extension, Router,
};
use config::{builder::DefaultState, ConfigBuilder, ConfigError, Environment, File};
use graphql_client::{reqwest::post_graphql, GraphQLQuery};
use manga_search_by_title::MangaSearchByTitleMangasNodes;
use reqwest;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tower_sessions::{cookie::time::Duration, Expiry, MemoryStore, Session, SessionManagerLayer};
use util::join_url;

mod util; // Declare the util module

#[derive(Deserialize)]
struct AppConfig {
    suwayomi_url: String,
    // Add other configuration fields here
}

impl AppConfig {
    pub fn new() -> Result<Self, ConfigError> {
        let builder = ConfigBuilder::<DefaultState>::default()
            .add_source(File::with_name("config/default.toml").required(false))
            .add_source(Environment::with_prefix("APP").separator("__"))
            .build()?;

        builder.try_deserialize::<AppConfig>()
    }
}

struct AppState {
    config: RwLock<AppConfig>, // Use RwLock for thread-safe access
}

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

// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/queries/MangaSearchByTitle.graphql",
    response_derives = "Debug"
)]
pub struct MangaSearchByTitle;

async fn search_manga_by_title(
    variables: manga_search_by_title::Variables,
    base_url: &str,
) -> Result<Vec<manga_search_by_title::MangaSearchByTitleMangasNodes>, Error> {
    let client = reqwest::Client::new();

    return match post_graphql::<MangaSearchByTitle, _>(
        &client,
        join_url(base_url, "/api/graphql")?,
        variables,
    )
    .await?
    .data
    {
        Some(data) => Ok(data.mangas.nodes),
        None => Err(anyhow!("Missing response data")),
    };
}

#[derive(Template)]
#[template(path = "search-results.html")]
struct SearchResultsTemplate {
    title: String,
    mangas: Vec<MangaSearchByTitleMangasNodes>,
    api_base: String,
}

#[debug_handler]
async fn search_results(
    Extension(shared_state): Extension<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<SearchResultsTemplate, AppError> {
    let api_base = &shared_state.config.read().await.suwayomi_url;
    let title = params.get("title").unwrap().to_string();
    let res = search_manga_by_title(
        manga_search_by_title::Variables {
            title: title.clone(),
        },
        api_base,
    )
    .await?;
    Ok(SearchResultsTemplate {
        title: title,
        mangas: res,
        api_base: api_base.clone(),
    })
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/queries/SpecificMangaById.graphql",
    response_derives = "Debug"
)]
pub struct SpecificMangaById;

async fn get_manga_by_id(
    id: i64,
    base_url: &str,
) -> Result<specific_manga_by_id::SpecificMangaByIdManga, Error> {
    let client = reqwest::Client::new();

    return match post_graphql::<SpecificMangaById, _>(
        &client,
        join_url(base_url, "/api/graphql")?,
        specific_manga_by_id::Variables { id },
    )
    .await?
    .data
    {
        Some(data) => Ok(data.manga),
        None => Err(anyhow!("Missing response data")),
    };
}

#[derive(Template)]
#[template(path = "manga-page.html")]
struct MangaPageTemplate {
    manga: specific_manga_by_id::SpecificMangaByIdManga,
    api_base: String,
}

#[debug_handler]
async fn manga_by_id(
    Extension(shared_state): Extension<Arc<AppState>>,
    params: Path<i64>,
) -> Result<MangaPageTemplate, AppError> {
    let api_base = &shared_state.config.read().await.suwayomi_url;
    let manga = get_manga_by_id(params.0, api_base).await?;
    Ok(MangaPageTemplate {
        manga,
        api_base: api_base.clone(),
    })
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/queries/SpecificMangaChapters.graphql",
    response_derives = "Debug,Clone"
)]
pub struct SpecificMangaChapters;

async fn get_chapters_by_id(
    id: i64,
    base_url: &str,
) -> Result<
    (
        String,
        Vec<specific_manga_chapters::SpecificMangaChaptersMangaChaptersNodes>,
    ),
    Error,
> {
    let client = reqwest::Client::new();

    return match post_graphql::<SpecificMangaChapters, _>(
        &client,
        join_url(base_url, "/api/graphql")?,
        specific_manga_chapters::Variables { id },
    )
    .await?
    .data
    {
        Some(data) => Ok((data.manga.title, data.manga.chapters.nodes)),
        None => Err(anyhow!("Missing response data")),
    };
}

#[derive(Template)]
#[template(path = "chapter-select.html")]
struct ChapterSelectTemplate {
    mangaId: i64,
    title: String,
    items: Vec<specific_manga_chapters::SpecificMangaChaptersMangaChaptersNodes>,
    selected: HashSet<i64>,
    limit: usize,
    offset: usize,
}

#[debug_handler]
async fn get_chapters_by_manga_id(
    Extension(shared_state): Extension<Arc<AppState>>,
    params: Path<i64>,
) -> Result<ChapterSelectTemplate, AppError> {
    let api_base = &shared_state.config.read().await.suwayomi_url;
    let (title, chapters) = get_chapters_by_id(params.0, api_base).await?;
    let limit = 20;
    let offset = 0;
    // CQ: TODO avoid this copy
    let items = chapters[offset..limit].to_vec();
    Ok(ChapterSelectTemplate {
        mangaId: params.0,
        title,
        items,
        limit,
        offset,
        selected: HashSet::new(),
    })
}

#[derive(Deserialize)]
struct ChapterSelectInput {
    #[serde(
        rename = "selected_items[]",
        flatten,
        deserialize_with = "util::deserialize_items"
    )]
    selected_items: Vec<String>,
    page_control: Option<String>,
}

const SESSION_OFFSET_KEY: &str = "offset";
#[derive(Default, Deserialize, Serialize)]
struct SessionOffset(usize);
const SESSION_SELECTED_CHAPTERS_KEY: &str = "selected_chapters";
#[derive(Default, Deserialize, Serialize)]
struct SessionSelectedChapters(HashMap<usize, Vec<i64>>);

#[debug_handler]
async fn post_chapters_by_manga_id(
    Extension(shared_state): Extension<Arc<AppState>>,
    params: Path<i64>,
    session: Session,
    Form(data): Form<ChapterSelectInput>,
) -> Result<ChapterSelectTemplate, AppError> {
    let api_base = &shared_state.config.read().await.suwayomi_url;

    let limit = 20;
    let mut offset: SessionOffset = session
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
        None => prev_page_offset,
    };

    let mut previously_selected_chapters: HashSet<i64> = HashSet::new();

    match session_selected_chapters.get(&new_current_page) {
        // CQ: TODO avoid this copy
        Some(v) => {
            for &value in v {
                previously_selected_chapters.insert(value);
            }
        }
        None => {}
    };

    session_selected_chapters.insert(prev_page_offset, new_chapters_selected);
    session
        .insert(
            SESSION_SELECTED_CHAPTERS_KEY,
            SessionSelectedChapters(session_selected_chapters),
        )
        .await?;

    session
        .insert(SESSION_OFFSET_KEY, new_current_page)
        .await
        .unwrap();

    offset = session
        .get(SESSION_OFFSET_KEY)
        .await
        .unwrap()
        .unwrap_or_default();

    let end = offset.0 + limit;

    let (title, chapters) = get_chapters_by_id(params.0, api_base).await?;
    // CQ: TODO avoid this copy
    let items = chapters[offset.0..end].to_vec();

    Ok(ChapterSelectTemplate {
        mangaId: params.0,
        title,
        items,
        limit,
        offset: new_current_page,
        selected: previously_selected_chapters,
    })
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
    let config = AppConfig::new().expect("Failed to load configuration");
    let state = Arc::new(AppState {
        config: RwLock::new(config),
    });

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
        .layer(Extension(state.clone()))
        .layer(session_layer);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
