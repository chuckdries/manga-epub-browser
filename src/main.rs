use std::{collections::HashMap, sync::Arc};

use anyhow::{anyhow, Error, Result};
use askama::Template;
use axum::{
    debug_handler,
    extract::{Path, Query},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Extension, Router,
};
use graphql_client::{reqwest::post_graphql, GraphQLQuery};
use manga_search_by_title::MangaSearchByTitleMangasNodes;
use reqwest;

use config::{builder::DefaultState, ConfigBuilder, ConfigError, Environment, File};
use serde::Deserialize;
use tokio::sync::RwLock;
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
        Some(data) => {
            println!("{:#?}", data.mangas.nodes);
            Ok(data.mangas.nodes)
        }
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
    response_derives = "Debug"
)]
pub struct SpecificMangaChapters;

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
    // build our application with a single route
    let app = Router::new()
        .route("/", get(home))
        .route("/search", get(search_results))
        .route("/manga/:id", get(manga_by_id))
        .fallback(not_found)
        .layer(Extension(state.clone()));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
