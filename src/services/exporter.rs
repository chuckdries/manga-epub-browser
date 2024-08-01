use std::sync::Arc;

use eyre::eyre;
use sqlx::SqlitePool;

use crate::{
    models::{
        export::{
            get_export_by_id, get_export_chapters_by_id,
            set_export_state, Export, ExportFormat, ExportState, ExportStep,
        },
        export_log::log_export_step,
    }, services::assemblers::cbz::assemble_cbz, suwayomi::{download_chapters_from_source, fetch_chapters_from_suwayomi}, AppError
};

use super::assemblers::epub::assemble_epub;

static STEPS: [ExportStep; 5] = [
    ExportStep::Begin,
    ExportStep::DownloadingFromSource,
    ExportStep::FetchingFromSuwayomi,
    ExportStep::AssemblingFile,
    ExportStep::Complete,
];

// TODO log stuff
async fn execute_export(pool: Arc<SqlitePool>, id: i64) -> Result<(), AppError> {
    let mut export = get_export_by_id(&*pool, id)
        .await?
        .ok_or(eyre!("Export not found"))?;
    if export.state == ExportState::Completed {
        return Ok(());
    }

    for step in STEPS
        .iter()
        .skip(STEPS.iter().position(|&s| s == export.step).unwrap_or(0))
    {
        let _ = log_export_step(&*pool, export.id, export.step, "Starting step").await;
        perform_export_step(pool.clone(), &mut export, *step).await?;
        set_export_state(&*pool, id, &export.state, &export.step).await?;
        let _ = log_export_step(&*pool, export.id, export.step, "Finished step").await;
    }
    Ok(())
}

async fn perform_export_step(
    pool: Arc<SqlitePool>,
    export: &mut Export,
    step: ExportStep,
) -> Result<(), AppError> {
    match step {
        ExportStep::Begin => {
            export.state = ExportState::InProgress;
            export.step = ExportStep::DownloadingFromSource;
        }
        ExportStep::DownloadingFromSource => {
            let ids = get_export_chapters_by_id(&*pool, export.id).await?;
            download_chapters_from_source(&ids).await?;
            export.step = ExportStep::FetchingFromSuwayomi;
        }
        ExportStep::FetchingFromSuwayomi => {
            let ids = get_export_chapters_by_id(&*pool, export.id).await?;
            fetch_chapters_from_suwayomi(&ids).await?;
            export.step = ExportStep::AssemblingFile;
        }
        ExportStep::AssemblingFile => {
            let chapters = get_export_chapters_by_id(&*pool, export.id).await?;
            match export.format {
                ExportFormat::Epub => {
                    assemble_epub(pool, export, &chapters).await?;
                }
                ExportFormat::Cbz => {
                    match assemble_cbz(pool, export, &chapters).await {
                        Ok(_) => {}
                        Err(e) => {
                            dbg!(&e);
                            return Err(e);
                        }
                    };
                }
            }
            export.step = ExportStep::Complete;
        }
        ExportStep::Complete => {
            export.state = ExportState::Completed;
        }
    }
    Ok(())
}

pub async fn resume_interrupted_exports(pool: Arc<SqlitePool>) -> Result<(), AppError> {
    let exports = sqlx::query!(
        r#"
        SELECT id FROM Export WHERE state = ?
        "#,
        ExportState::InProgress
    )
    .fetch_all(&*pool)
    .await?;

    if exports.is_empty() {
        return Ok(());
    }

    for export in exports {
        dbg!("resuming export {}", export.id);
        let pool_clone = pool.clone();
        tokio::spawn(async move { execute_export(pool_clone, export.id).await });
    }

    Ok(())
}

pub async fn begin_export(pool: Arc<SqlitePool>, id: i64) -> Result<(), AppError> {
    tokio::spawn(async move { execute_export(pool, id).await });
    Ok(())
}
