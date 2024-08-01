use std::{collections::HashSet, sync::Arc};

use anyhow::Result;
use askama::Template;
use axum::{extract::Query, response::Redirect, Extension};
use axum_extra::extract::Form;
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{
    services::export::{create_export, set_chapters_for_export},
    suwayomi::{self, specific_manga_chapters::SpecificMangaChaptersMangaChaptersNodes},
    AppError,
};

#[derive(Template)]
#[template(path = "better-chapter-select.html")]
pub struct ChapterSelect {
    chapters: Vec<SpecificMangaChaptersMangaChaptersNodes>,
    manga_id: i64,
    hide_read: bool,
}

#[derive(Deserialize)]
pub struct ChapterSelectParams {
    manga_id: i64,
    hide_read: Option<bool>,
}

#[axum::debug_handler]
pub async fn view_chapter_select(
    Query(params): Query<ChapterSelectParams>,
) -> Result<ChapterSelect, AppError> {
    let manga_id = params.manga_id;
    let all_chapters = suwayomi::get_chapters_by_manga_id(manga_id).await?;
    let (chapters, hide_read) = match params.hide_read {
        Some(true) => (
            all_chapters
                .into_iter()
                .filter(|chapter| !chapter.is_read)
                .collect(),
            true,
        ),
        _ => (all_chapters, false),
    };
    Ok(ChapterSelect {
        chapters,
        manga_id,
        hide_read,
    })
}

#[derive(Deserialize)]
pub struct ChapterSelectSubmission {
    chapter_id: HashSet<i64>,
    manga_id: i64,
}

#[axum::debug_handler]
pub async fn post_chapter_select(
    Extension(pool): Extension<Arc<SqlitePool>>,
    Form(params): Form<ChapterSelectSubmission>,
) -> Result<Redirect, AppError> {
    let manga = suwayomi::get_manga_by_id(params.manga_id).await?;
    let author = manga.author.unwrap_or("Unknown".to_string());

    let export = create_export(&*pool, &manga.title, &author).await?;
    set_chapters_for_export(&pool, export, params.chapter_id).await?;
    Ok(Redirect::to(&format!("/export/{}/configure", export)))
}
