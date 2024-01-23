use std::collections::HashMap;

use anyhow::{anyhow, Error, Result};
use askama::Template;
use axum::{
    debug_handler,
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response, Html},
    routing::get,
    Router,
};
use graphql_client::{reqwest::post_graphql, GraphQLQuery};
use manga_search_by_title::MangaSearchByTitleMangasNodes;
use reqwest;
// use serde::Serialize;

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
) -> Result<Vec<manga_search_by_title::MangaSearchByTitleMangasNodes>, Error> {
    let client = reqwest::Client::new();

    return match post_graphql::<MangaSearchByTitle, _>(
        &client,
        "http://10.10.11.250:4567/api/graphql",
        variables,
    )
    .await?
    .data
    {
        Some(data) => {
            println!("{:#?}", data.mangas.nodes);
            Ok(data.mangas.nodes)
        },
        None => Err(anyhow!("Missing response data")),
    };
}

#[derive(Template)]
#[template(path = "search-results.html")]
struct SearchResultsTemplate {
    title: String,
    mangas: Vec<MangaSearchByTitleMangasNodes>,
    api_base: &'static str,
}

#[debug_handler]
async fn search_results(
    Query(params): Query<HashMap<String, String>>,
) -> Result<SearchResultsTemplate, AppError> {
    let title = params.get("title").unwrap().to_string();
    let res = search_manga_by_title(manga_search_by_title::Variables {
        title: title.clone(),
    })
    .await?;
    Ok(SearchResultsTemplate {
        title: title,
        mangas: res,
        api_base: "http://10.10.11.250:4567"
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
) -> Result<specific_manga_by_id::SpecificMangaByIdManga, Error> {
    let client = reqwest::Client::new();

    return match post_graphql::<SpecificMangaById, _>(
        &client,
        "http://10.10.11.250:4567/api/graphql",
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
    api_base: &'static str,
}

#[debug_handler]
async fn manga_by_id(
    params: Path<i64>
) -> Result<MangaPageTemplate, AppError> {
    let manga = get_manga_by_id(params.0).await?;
    Ok(MangaPageTemplate{ manga, api_base: "http://10.10.11.250:4567" })
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
    // build our application with a single route
    let app = Router::new()
        .route("/", get(home))
        .route("/search", get(search_results))
        .route("/manga/:id", get(manga_by_id))
        .fallback(not_found);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
