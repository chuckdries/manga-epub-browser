use askama::Template;
use graphql_client::{GraphQLQuery, Response};
use axum::{
    debug_handler,
    extract::Path,
    routing::get,
    Router,
};
use serde::Serialize;
use std::error::Error;
use reqwest;

// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/queries/MangaSearchByTitle.graphql",
    request_derives = "Debug",
    response_derives = "Debug",
)]
pub struct MangaSearchByTitle;

async fn search_manga_by_title(variables: manga_search_by_title::Variables) -> Result<manga_search_by_title::ResponseData, Box<dyn Error>> {

    println!("hello");
    // this is the important line
    let request_body = MangaSearchByTitle::build_query(variables);

    let client = reqwest::Client::new();
    let mut res = client.post("http://10.10.11.250:4567/api/graphql").json(&request_body).send().await?;
    let response_body: Response<manga_search_by_title::ResponseData> = res.json().await?;
    println!("res: {:#?}", response_body);
    Ok(response_body.data.expect("empty response"))
}

#[derive(Template)]
#[template(path = "hello.html")]
struct HelloTemplate {
    name: String,
}

#[debug_handler]
async fn hello(Path(name): Path<String>) -> HelloTemplate {
    let res = search_manga_by_title(manga_search_by_title::Variables { title: "Frieren".to_string() }).await.unwrap();
    println!("{:#?}", res.mangas.nodes.len());
    HelloTemplate { name }
}


#[tokio::main]
async fn main() {
    // build our application with a single route
    let app = Router::new().route("/:name", get(hello));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}