extern crate dotenv;
use anyhow::{
    anyhow,
    // Error,
    Result,
};
use askama::Template;
use axum::{
    debug_handler,
    extract::{
        Path,
        Query,
        // State
    },
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Extension, Router,
};
use axum_extra::extract::Form;
use ebook::{
    commit_chapter_selection, get_book_by_id, get_book_table, get_book_with_chapters_by_id,
    update_book_details, BookStatus, SqlBook,
};
use serde::{Deserialize, Serialize};
use serde_json::{
    // json,
    Value,
};
use sqlx::SqlitePool;
use std::{
    collections::{HashMap, HashSet},
    i64::MAX,
    // net::SocketAddr,
    sync::Arc,
};
// use suwayomi::download_chapters;
// use suwayomi::get_chapters_by_id;
use dotenv::dotenv;
use std::env;
use suwayomi::{
    download_chapters,
    get_all_sources_by_lang,
    get_chapters_by_ids,
    get_manga_by_id, // specific_manga_by_id,
};
// use tokio::sync::RwLock;
use handlebars::{handlebars_helper, DirectorySourceOptions, Handlebars};
use tower_http::services::ServeDir;
use tower_sessions::{cookie::time::Duration, Expiry, MemoryStore, Session, SessionManagerLayer};

mod ebook;
mod suwayomi;
mod util; // Declare the util module

extern crate pretty_env_logger;
// #[macro_use]
extern crate log;

// struct AppState {
//     config: RwLock<AppConfig>, // Use RwLock for thread-safe access
// }

// CQ clean up this error handling mess
// Make our own error that wraps `anyhow::Error`.
struct AppError(anyhow::Error);

type AppResponse = Result<Html<String>, AppError>;

fn render<T: Serialize>(hbs: &Handlebars<'static>, name: &str, data: &T) -> AppResponse {
    match hbs.render(name, data) {
        Ok(rendered) => Ok(Html(rendered)),
        Err(msg) => Err(AppError(anyhow!(msg))),
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

// #[derive(Template)]
// #[template(path = "search-results.html")]
#[derive(Serialize)]
struct SearchResultsTemplate {
    title: String,
    mangas: Vec<suwayomi::manga_source_search::MangaSourceSearchFetchSourceMangaMangas>,
    api_base: String,
}

#[debug_handler]
async fn search_results(
    Query(params): Query<HashMap<String, String>>,
    handlebars: Extension<Arc<Handlebars<'static>>>,
) -> AppResponse {
    let title = params.get("title").unwrap().to_string();
    let sources = get_all_sources_by_lang(suwayomi::all_sources_by_language::Variables {
        lang: "en".to_string(),
    })
    .await
    .expect("configure sources and language before using search");
    // todo: search through all sources
    let res = suwayomi::search_manga_by_title(suwayomi::manga_source_search::Variables {
        input: suwayomi::manga_source_search::FetchSourceMangaInput {
            type_: suwayomi::manga_source_search::FetchSourceMangaType::SEARCH,
            client_mutation_id: None,
            query: Some(title.clone()),
            filters: Box::new(None),
            page: 1,
            source: sources.first().expect("no sources").id.clone(),
        },
    })
    .await?;

    let data = SearchResultsTemplate {
        title,
        mangas: res,
        api_base: env::var("SUWAYOMI_URL")?,
    };

    render(&handlebars, "search-results", &data)
    // Ok(SearchResultsTemplate {
    //     title: title,
    //     mangas: res,
    //     api_base: env::var("SUWAYOMI_URL")?,
    // })
}

#[derive(Template)]
#[template(path = "manga-page.html")]
struct MangaPageTemplate {
    manga: suwayomi::specific_manga_by_id::SpecificMangaByIdManga,
    api_base: String,
}

#[debug_handler]
async fn manga_by_id(params: Path<i64>) -> Result<MangaPageTemplate, AppError> {
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
async fn get_chapters_by_manga_id(params: Path<i64>) -> Result<ChapterSelectTemplate, AppError> {
    // CQ: TODO need to fetch chapters
    let (title, chapters) = suwayomi::get_chapters_by_manga_id(params.0).await?;
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
    let manga_id = params.0;
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

            let manga = get_manga_by_id(manga_id).await?;
            let default_author = match manga.author {
                Some(author) => author,
                None => "".to_string(),
            };

            let chapters = get_chapters_by_ids(&all_chapters)
                .await?
                .expect("Couldn't find details on selected chapter");

            let default_title = if chapters.nodes.len() == 1 {
                format!("{} ({})", manga.title, chapters.nodes[0].chapter_number)
            } else {
                let (min, max): (f64, f64) =
                    chapters
                        .nodes
                        .iter()
                        .fold((MAX as f64, 0_f64), |acc, chap| {
                            let num: f64 = chap.chapter_number;
                            if num < acc.0 {
                                return (num, acc.1);
                            }
                            if num > acc.1 {
                                return (acc.0, num);
                            }
                            return acc;
                        });
                format!("{} ({}-{})", manga.title, min, max)
            };

            let book_id = commit_chapter_selection(
                pool,
                all_chapters,
                manga_id,
                &default_title,
                &default_author,
            )
            .await?;

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

    let (title, chapters) = suwayomi::get_chapters_by_manga_id(manga_id).await?;

    let mut end = new_current_page + limit;
    let total_chapters = chapters.len();
    if end > total_chapters {
        end = total_chapters;
    }
    // CQ: TODO avoid this copy
    let items = chapters[new_current_page..end].to_vec();

    Ok(PostChapterResponse::TemplateResponse(
        ChapterSelectTemplate {
            manga_id,
            title,
            items,
            limit,
            offset: new_current_page,
            selected: previously_selected_chapters,
        },
    ))
}

#[derive(Template)]
#[template(path = "configure-book.html")]
struct ConfigureBookTemplate {
    id: i64,
    default_title: String,
    default_author: String,
}

#[debug_handler]
async fn get_configure_book(
    params: Path<i64>,
    Extension(pool): Extension<SqlitePool>,
) -> Result<ConfigureBookTemplate, AppError> {
    let book = match get_book_by_id(pool, params.0).await? {
        Some(book) => Ok(book),
        None => Err(anyhow!("Book not found")),
    }?;

    Ok(ConfigureBookTemplate {
        id: params.0,
        default_author: book.author,
        default_title: book.title,
    })
}

#[derive(Deserialize)]
struct ConfigureBookInput {
    title: String,
    author: String,
}

#[debug_handler]
async fn post_configure_book(
    params: Path<i64>,
    Extension(pool): Extension<SqlitePool>,
    Form(data): Form<ConfigureBookInput>,
) -> Result<impl IntoResponse, AppError> {
    let book = match get_book_with_chapters_by_id(&pool, params.0).await? {
        Some(book) => Ok(book),
        None => Err(anyhow!("Book not found")),
    }?;
    update_book_details(&pool, params.0, &data.title, &data.author).await?;
    tokio::spawn(async move { match download_chapters(book.chapters, book.book.id, &pool).await {
        Ok(()) => (),
        Err(e) => println!("{:#?}", e.0)
    } });
    Ok(Redirect::to(&format!("/")))
}

// #[template(path = "home.html")]
#[derive(Serialize)]
struct HomeTemplate {
    sources: Vec<suwayomi::all_sources_by_language::AllSourcesByLanguageSourcesNodes>,
    books: Vec<SqlBook>,
}

#[debug_handler]
async fn home(
    handlebars: Extension<Arc<Handlebars<'static>>>,
    Extension(pool): Extension<SqlitePool>,
) -> Result<Html<String>, AppError> {
    let sources = get_all_sources_by_lang(suwayomi::all_sources_by_language::Variables {
        lang: "en".to_string(),
    })
    .await?;
    let books = get_book_table(&pool).await?;
    match handlebars.render("home", &HomeTemplate { sources, books }) {
        Ok(rendered) => Ok(Html(rendered)),
        Err(msg) => Err(AppError(anyhow!(msg))),
    }
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

    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_expiry(Expiry::OnInactivity(Duration::days(14)));

    let mut handlebars = Handlebars::new();
    handlebars.set_dev_mode(true);

    handlebars_helper!(json: |v: Value| v.to_string());
    handlebars.register_helper("json", Box::new(json));

    handlebars_helper!(bookStatus: |v: u8| match v {
        1 => "Draft",
        2 => "Downloading",
        3 => "Assembling",
        4 => "Done",
        _ => "Errored"
    });
    handlebars.register_helper("bookStatus", Box::new(bookStatus));

    match handlebars.register_templates_directory(
        "templates",
        DirectorySourceOptions {
            tpl_extension: ".hbs".to_string(),
            hidden: false,
            temporary: false,
        },
    ) {
        Ok(()) => println!("templates loaded"),
        Err(msg) => panic!("{}", msg),
    };

    // handlebars.register_script_helper_file(name, script_path)

    let handlebars = Arc::new(handlebars);

    // build our application with a single route
    let app = Router::new()
        .route("/", get(home))
        .route("/search", get(search_results))
        .route("/manga/:id", get(manga_by_id))
        .route("/manga/:id/chapters", get(get_chapters_by_manga_id))
        .route("/manga/:id/chapters", post(post_chapters_by_manga_id))
        .route("/configure-book/:id", get(get_configure_book))
        .route("/configure-book/:id", post(post_configure_book))
        .nest_service("/public", ServeDir::new("public"))
        .fallback(not_found)
        .layer(Extension(pool))
        .layer(Extension(handlebars))
        .layer(session_layer);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
