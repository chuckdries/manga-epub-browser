use std::collections::HashMap;

use anyhow::{anyhow, Error};
use askama::Template;
use axum::{
    debug_handler,
    extract::{Path, Query},
    routing::get,
    Router,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use graphql_client::{reqwest::post_graphql, GraphQLQuery};
use manga_search_by_title::MangaSearchByTitleMangasNodes;
use reqwest;
use serde::Serialize;

// Make our own error that wraps `anyhow::Error`.
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
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



// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/queries/MangaSearchByTitle.graphql",
    request_derives = "Debug",
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
        Some(data) => Ok(data.mangas.nodes),
        None => Err(anyhow!("Missing response data")),
    };
}

#[derive(Template)]
#[template(path = "search-results.html")]
struct SearchResultsTemplate {
    title: String,
    mangas: Vec<MangaSearchByTitleMangasNodes>,
}

#[debug_handler]
async fn search_results(Query(params): Query<HashMap<String, String>>) -> Result<SearchResultsTemplate, AppError> {
    let title = params.get("title").unwrap().to_string();
    let res = search_manga_by_title(manga_search_by_title::Variables {
        title: title.clone(),
    })
    .await
    .unwrap();
    println!("results {:#?}", res);
    Ok(SearchResultsTemplate { title: title, mangas: res })
}

#[tokio::main]
async fn main() {
    // build our application with a single route
    let app = Router::new().route("/search", get(search_results));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
