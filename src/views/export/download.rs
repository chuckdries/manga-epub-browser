use std::{fmt::Debug, sync::Arc};

use askama_axum::IntoResponse;
use axum::{debug_handler, extract::{Path, Request}, Extension};
use eyre::eyre;
use sqlx::{pool::maybe, SqlitePool};
use tower::ServiceExt;
use tower_http::services::ServeFile;

use crate::{models::export::{get_export_by_id, ExportState}, AppError};

#[debug_handler]
pub async fn serve_export(
    Path(id): Path<i64>,
    Extension(pool): Extension<Arc<SqlitePool>>,
    request: Request
) -> Result<impl IntoResponse, (axum::http::StatusCode, String)> {
    let maybe_export = match get_export_by_id(&*pool, id).await {
        Ok(export) => Ok(export),
        Err(e) => Err((axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }?;
    let export = match maybe_export {
        Some(export) => Ok(export),
        None => Err((axum::http::StatusCode::NOT_FOUND, "Export not found".into())),
    }?;
    if export.state != ExportState::Completed {
        return Err((axum::http::StatusCode::BAD_REQUEST, "Export not completed".into()));
    }
    dbg!("request for export", &export);
    let path = export.get_path();
    dbg!("serving file", &path);
    Ok(ServeFile::new(path).oneshot(request).await)
}