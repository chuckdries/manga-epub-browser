use std::sync::Arc;

use askama::Template;
use askama_axum::IntoResponse;
use axum::{
    extract::Path,
    response::{Redirect, Response},
    Extension,
};
use axum_extra::extract::Form;
use eyre::eyre;
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{
    services::book_compiler::begin_compile_book,
    models::export::{Export, ExportFormat, ExportState, get_export_and_chapters_by_id, set_export_config},
    suwayomi::{self, chapters_by_ids::ChaptersByIdsChaptersNodes},
    AppError,
};

#[derive(Template)]
#[template(path = "export-configure.html")]
pub struct ExportConfigure {
    export: Export,
    chapters: Vec<ChaptersByIdsChaptersNodes>,
}

#[axum::debug_handler]
pub async fn view_configure_book(
    Extension(pool): Extension<Arc<SqlitePool>>,
    Path(id): Path<i64>,
) -> Result<Response, AppError> {
    let (export, chapters) = match get_export_and_chapters_by_id(&*pool, id).await? {
        Some(export_and_chapters) => Ok(export_and_chapters),
        None => Err(AppError(eyre!("Export not found"))),
    }?;

    if export.state != ExportState::Draft {
        return Err(AppError(eyre!("Export is not in draft state")));
    }
    let chapter_details = match suwayomi::get_chapters_by_ids(&chapters).await? {
        Some(chapter_details) => Ok(chapter_details),
        None => Err(AppError(eyre!("Chapters not found"))),
    }?;
    Ok(ExportConfigure {
        export,
        chapters: chapter_details.nodes,
    }
    .into_response())
}

#[derive(Deserialize)]
pub struct ConfigureExportInput {
    title: String,
    author: String,
    format: ExportFormat,
    action: String,
}

pub async fn post_configure_export(
    Extension(pool): Extension<Arc<SqlitePool>>,
    Path(id): Path<i64>,
    Form(data): Form<ConfigureExportInput>,
) -> Result<Response, AppError> {
    set_export_config(&pool, id, &data.title, &data.author, data.format).await?;
    // do this to render template
    // view_configure_book(Extension(pool), Path(id)).await
    if data.action == "save" {
        return Ok(Redirect::to("/exports").into_response());
    }
    begin_compile_book(pool, id).await?;
    Ok(Redirect::to(&format!("/export/{}", id)).into_response())
}
