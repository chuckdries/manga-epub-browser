use std::sync::Arc;

use askama::Template;
use axum::Extension;
use sqlx::SqlitePool;

use crate::{
    services::export::{get_export_list, Export},
    AppError,
};

#[derive(Template)]
#[template(path = "export-list.html")]
pub struct ExportList {
    exports: Vec<Export>,
}

#[axum::debug_handler]
pub async fn view_export_list(
    Extension(pool): Extension<Arc<SqlitePool>>,
) -> Result<ExportList, AppError> {
    let exports = get_export_list(&*pool).await?;
    Ok(ExportList { exports })
}
