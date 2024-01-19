use askama::Template;
use axum::{
    debug_handler,
    extract::Path,
    routing::get,
    Router,
};

#[derive(Template)]
#[template(path = "hello.html")]
struct HelloTemplate {
    name: String,
}

#[debug_handler]
async fn hello(Path(name): Path<String>) -> HelloTemplate {
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