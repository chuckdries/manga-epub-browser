use std::{collections::HashSet, env};

use anyhow::{anyhow, Error, Result};
use graphql_client::{reqwest::post_graphql, GraphQLQuery};
use tokio::time::{sleep, Duration};

use crate::{suwayomi::check_on_download_progress::DownloaderState, util::join_url, AppError};

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/schema.json",
    query_path = "graphql/queries/MangaSearchByTitle.graphql",
    response_derives = "Debug,Serialize"
)]
pub struct MangaSearchByTitle;

pub async fn search_manga_by_title(
    variables: manga_search_by_title::Variables,
) -> Result<Vec<manga_search_by_title::MangaSearchByTitleMangasNodes>, Error> {
    let client = reqwest::Client::new();

    return match post_graphql::<MangaSearchByTitle, _>(
        &client,
        join_url(&env::var("SUWAYOMI_URL")?, "/api/graphql")?,
        variables,
    )
    .await?
    .data
    {
        Some(data) => Ok(data.mangas.nodes),
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
        join_url(&env::var("SUWAYOMI_URL")?, "/api/graphql")?,
        specific_manga_chapters::Variables { id },
    )
    .await?
    .data
    {
        Some(data) => Ok((data.manga.title, data.manga.chapters.nodes)),
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

pub async fn download_chapters(ids: HashSet<i64>) -> Result<(), Error> {
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
        return Ok(());
    }

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
            Some(data) => Ok(data.download_status.state),
            None => Err(anyhow!("Missing response data")),
        }?;

        dbg!(&downloader_state);

        if downloader_state == DownloaderState::STOPPED {
            break;
        }

        // Wait for a specified interval before polling again
        sleep(Duration::from_secs(5)).await;
    }

    println!("download complete");

    Ok(())
}

// CQ: TODO
pub async fn get_chapters_by_ids(ids: HashSet<i64>) -> Result<(), AppError> {
    Ok(())
}