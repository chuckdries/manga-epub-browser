use std::io::{copy, Cursor};
use std::{collections::HashSet, env};

use anyhow::{anyhow, Error, Result};
use graphql_client::{reqwest::post_graphql, GraphQLQuery};
use sqlx::SqlitePool;
use tokio::time::{sleep, Duration};

use futures::future::join_all;
use futures::prelude::*;
use regex::Regex;

use crate::{
    ebook::{update_book_status, BookStatus},
    suwayomi::check_on_download_progress::DownloaderState,
    util::join_url,
    AppError,
};

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/queries/GetLibrary.graphql",
    response_derives = "Debug,Serialize"
)]
pub struct GetLibrary;

pub async fn get_library() -> Result<Vec<get_library::GetLibraryMangasNodes>, Error> {
    let client = reqwest::Client::new();

    return match post_graphql::<GetLibrary, _>(
        &client,
        join_url(&env::var("SUWAYOMI_URL")?, "/api/graphql")?,
        get_library::Variables {},
    )
    .await?
    .data
    {
        Some(data) => Ok(data.mangas.nodes),
        None => Ok(Vec::new()),
    };
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/queries/AllSourcesByLanguage.graphql",
    response_derives = "Debug,Serialize"
)]
pub struct AllSourcesByLanguage;

pub async fn get_all_sources_by_lang(
    variables: all_sources_by_language::Variables,
) -> Result<Vec<all_sources_by_language::AllSourcesByLanguageSourcesNodes>, Error> {
    let client = reqwest::Client::new();

    return match post_graphql::<AllSourcesByLanguage, _>(
        &client,
        join_url(&env::var("SUWAYOMI_URL")?, "/api/graphql")?,
        variables,
    )
    .await?
    .data
    {
        Some(data) => Ok(data.sources.nodes),
        None => Ok(Vec::new()),
    };
}

type LongString = String;
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/queries/MangaSourceSearch.graphql",
    response_derives = "Debug,Serialize"
)]
pub struct MangaSourceSearch;

pub async fn search_manga_by_title(
    variables: manga_source_search::Variables,
) -> Result<Vec<manga_source_search::MangaSourceSearchFetchSourceMangaMangas>, Error> {
    let client = reqwest::Client::new();

    return match post_graphql::<MangaSourceSearch, _>(
        &client,
        join_url(&env::var("SUWAYOMI_URL")?, "/api/graphql")?,
        variables,
    )
    .await?
    .data
    {
        Some(data) => Ok(data.fetch_source_manga.mangas),
        None => Err(anyhow!("Missing response data")),
    };
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/queries/SpecificMangaById.graphql",
    response_derives = "Debug"
)]
pub struct SpecificMangaById;

pub async fn get_manga_by_id(
    id: i64,
) -> Result<specific_manga_by_id::SpecificMangaByIdManga, Error> {
    let client = reqwest::Client::new();

    return match post_graphql::<SpecificMangaById, _>(
        &client,
        join_url(&env::var("SUWAYOMI_URL")?, "/api/graphql")?,
        specific_manga_by_id::Variables { id },
    )
    .await?
    .data
    {
        Some(data) => Ok(data.manga),
        None => Err(anyhow!("Missing response data")),
    };
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/queries/SpecificMangaChapters.graphql",
    response_derives = "Debug,Clone"
)]
pub struct SpecificMangaChapters;

pub async fn get_chapters_by_manga_id(
    id: i64,
) -> Result<Vec<specific_manga_chapters::SpecificMangaChaptersMangaChaptersNodes>, Error> {
    let client = reqwest::Client::new();

    return match post_graphql::<SpecificMangaChapters, _>(
        &client,
        join_url(&env::var("SUWAYOMI_URL")?, "/api/graphql")?,
        specific_manga_chapters::Variables { id },
    )
    .await?
    .data
    {
        Some(data) => Ok(data.manga.chapters.nodes),
        None => Err(anyhow!("Missing response data")),
    };
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/queries/CheckChaptersDownloaded.graphql",
    response_derives = "Debug,Clone"
)]
pub struct CheckChaptersDownloaded;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/queries/DownloadChapters.graphql",
    response_derives = "Debug,Clone"
)]
pub struct DownloadChapters;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/queries/CheckOnDownloadProgress.graphql",
    response_derives = "Debug,Clone,PartialEq"
)]
pub struct CheckOnDownloadProgress;

pub async fn download_chapters(
    ids: HashSet<i64>,
    book_id: i64,
    pool: &SqlitePool,
) -> Result<(), AppError> {
    let client = reqwest::Client::new();

    dbg!(&ids);
    let chapters_download_status = match post_graphql::<CheckChaptersDownloaded, _>(
        &client,
        join_url(&env::var("SUWAYOMI_URL")?, "/api/graphql")?,
        check_chapters_downloaded::Variables {
            ids: ids.into_iter().collect(),
        },
    )
    .await?
    .data
    {
        Some(data) => Ok(data.chapters.nodes),
        None => Err(anyhow!("Missing response data")),
    }?;

    let chapters_to_download: Vec<_> = chapters_download_status
        .iter()
        .filter(|n| !n.is_downloaded)
        .map(|n| n.id)
        .collect();

    dbg!(&chapters_to_download);

    if chapters_to_download.len() == 0 {
        println!("Skipped downloading - all chapters already downloaded");
        update_book_status(pool, book_id, BookStatus::ASSEMBLING).await?;
        // assembly procedure
        join_all(chapters_download_status.iter().map(|chap| {
            let path_prefix = format!("data{}", chap.url);
            fetch_chapter(chap.id, path_prefix)
        }))
        .await;

        print!("chapters fetched");
        return Ok(());
    }

    update_book_status(pool, book_id, BookStatus::DOWNLOADING).await?;

    let res = post_graphql::<DownloadChapters, _>(
        &client,
        join_url(&env::var("SUWAYOMI_URL")?, "/api/graphql")?,
        download_chapters::Variables {
            ids: chapters_to_download,
        },
    )
    .await?;

    dbg!(res);

    loop {
        let downloader_state = match post_graphql::<CheckOnDownloadProgress, _>(
            &client,
            join_url(&env::var("SUWAYOMI_URL")?, "/api/graphql")?,
            check_on_download_progress::Variables {},
        )
        .await?
        .data
        {
            Some(data) => Ok(data),
            None => Err(anyhow!("Missing response data")),
        }?;

        dbg!(&downloader_state);

        if downloader_state.download_status.state == DownloaderState::STOPPED {
            break;
        }

        // Wait for a specified interval before polling again
        sleep(Duration::from_secs(5)).await;
    }

    println!("download complete");

    update_book_status(pool, book_id, BookStatus::ASSEMBLING).await?;

    // assembly procedure
    join_all(chapters_download_status.iter().map(|chap| {
        let path_prefix = format!("data{}", chap.url);
        fetch_chapter(chap.id, path_prefix)
    }))
    .await;

    print!("chapters fetched");

    Ok(())
}

async fn dl_img(url: &str, path_prefix: &str) -> Result<(), AppError> {
    // let client = reqwest::Client::new();
    let re = Regex::new(r"/api/v1/manga/\d+/chapter/\d+/page/(\d+)").unwrap();
    let Some(caps) = re.captures(url) else {
        return Err(AppError(anyhow!("Couldn't parse image url")));
    };
    std::fs::create_dir_all(path_prefix)?;
    let mut file = std::fs::File::create(format!("{}/{}", path_prefix, &caps[0]))?;
    let response = reqwest::get(join_url(&env::var("SUWAYOMI_URL")?, url)?)
        .await
        .unwrap();
    let mut content = Cursor::new(response.bytes().await?);
    copy(&mut content, &mut file)?;
    Ok(())
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/queries/FetchChapterPages.graphql",
    response_derives = "Debug,Clone"
)]
pub struct FetchChapterPages;
pub async fn fetch_chapter(chapter: i64, path_prefix: String) -> Result<(), AppError> {
    let client = reqwest::Client::new();

    let urls = match post_graphql::<FetchChapterPages, _>(
        &client,
        join_url(&env::var("SUWAYOMI_URL")?, "/api/graphql")?,
        fetch_chapter_pages::Variables { id: chapter },
    )
    .await?
    .data
    {
        Some(data) => data.fetch_chapter_pages.pages,
        None => vec![],
    };

    join_all(urls.iter().map(|img| dl_img(img, &path_prefix))).await;
    Ok(())
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/queries/ChaptersByIds.graphql",
    response_derives = "Debug,Clone"
)]
pub struct ChaptersByIds;

pub async fn get_chapters_by_ids(
    ids: &HashSet<i64>,
) -> Result<Option<chapters_by_ids::ChaptersByIdsChapters>, AppError> {
    let client = reqwest::Client::new();

    match post_graphql::<ChaptersByIds, _>(
        &client,
        join_url(&env::var("SUWAYOMI_URL")?, "/api/graphql")?,
        chapters_by_ids::Variables {
            ids: Some(ids.into_iter().cloned().collect()),
        },
    )
    .await?
    .data
    {
        Some(data) => Ok(Some(data.chapters)),
        None => Ok(None),
    }
}
