use std::sync::Arc;

use askama::Template;
use axum::{debug_handler, extract::Path, Extension};
use eyre::eyre;
use sqlx::SqlitePool;

use crate::{
    models::export::{get_export_and_chapters_by_id, Export},
    suwayomi::get_chapters_by_ids,
    views::components::chapter_table::ChapterTable,
    AppError,
};

#[derive(Template)]
#[template(path = "export-details.html")]
pub struct ExportDetails {
    export: Export,
    chapter_table: ChapterTable,
}

// TODO task log
// TODO status from tasks table
#[debug_handler]
pub async fn view_book_details(
    Extension(pool): Extension<Arc<SqlitePool>>,
    Path(id): Path<i64>,
) -> Result<ExportDetails, AppError> {
    let (export, chapter_ids) = match get_export_and_chapters_by_id(&pool, id).await? {
        Some(book) => book,
        None => return Err(eyre!("Export not found").into()),
    };
    let chapters = match get_chapters_by_ids(&chapter_ids).await? {
        Some(chapters) => chapters.nodes,
        None => return Err(eyre!("Chapters not found").into()),
    };
    let template = ExportDetails {
        export,
        chapter_table: ChapterTable { chapters },
    };

    Ok(template)
}
