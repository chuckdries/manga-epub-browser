use std::{
    collections::HashSet,
    env,
    fs::{self, File},
    io::Cursor,
    path::Path,
    sync::Arc,
};

use epub_builder::{EpubBuilder, EpubContent, ZipLibrary};
use eyre::eyre;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use time::OffsetDateTime;

use crate::{
    ebook::{get_book_with_chapters_by_id, Book},
    suwayomi::{download_chapters_from_source, fetch_chapters_from_suwayomi, get_chapters_by_ids},
    AppError,
};

use super::task_log::log_task_step;

#[derive(Debug, Serialize, Deserialize, sqlx::Type, Clone, Copy, PartialEq)]
#[sqlx(rename_all = "snake_case")]
pub enum CompileTaskStep {
    Initialize,
    DownloadingFromSource,
    FetchingFromSuwayomi,
    AssemblingFile,
    Complete,
}

#[derive(Debug, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(rename_all = "snake_case")]
pub enum TaskState {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CompileTask {
    id: i64,
    state: TaskState,
    progress: i64,
    current_step: CompileTaskStep,
    date_created: OffsetDateTime,
    book_id: i64,
}

impl std::fmt::Display for CompileTaskStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileTaskStep::Initialize => write!(f, "Initialize"),
            CompileTaskStep::DownloadingFromSource => write!(f, "Downloading from source"),
            CompileTaskStep::FetchingFromSuwayomi => write!(f, "Fetching from Suwayomi"),
            CompileTaskStep::AssemblingFile => write!(f, "Assembling file"),
            CompileTaskStep::Complete => write!(f, "Complete"),
        }
    }
}

impl From<std::string::String> for CompileTaskStep {
    fn from(text: std::string::String) -> Self {
        match text.as_str() {
            "initialize" => CompileTaskStep::Initialize,
            "downloading_from_source" => CompileTaskStep::DownloadingFromSource,
            "fetching_from_suwayomi" => CompileTaskStep::FetchingFromSuwayomi,
            "assembling_file" => CompileTaskStep::AssemblingFile,
            "complete" => CompileTaskStep::Complete,
            _ => panic!("Invalid CompileTaskStep"),
        }
    }
}

pub async fn begin_compile_book(pool: Arc<SqlitePool>, book_id: i64) -> Result<(), AppError> {
    tokio::spawn(async move {
        let id = match create_task(pool.clone(), book_id).await {
            Ok(id) => id,
            Err(e) => {
                log_task_step(
                    &*pool,
                    0,
                    CompileTaskStep::Initialize,
                    &format!("Failed to create task: {:#?}", e),
                )
                .await
                .unwrap();
                return;
            }
        };
        execute_task(pool, id).await;
    });

    Ok(())
}

pub async fn resume_interrupted_tasks(pool: Arc<SqlitePool>) -> Result<(), AppError> {
    let tasks = sqlx::query_as!(
        CompileTask,
        r#"SELECT 
            id,
            state as "state: TaskState",
            progress,
            book_id,
            current_step as "current_step: CompileTaskStep",
            date_created as "date_created: OffsetDateTime"
            FROM CompileTasks WHERE state = ? OR state = ?"#,
        TaskState::InProgress,
        TaskState::Pending
    )
    .fetch_all(&*pool)
    .await?;

    for task in tasks {
        let pool_clone = pool.clone();
        log_task_step(
            &*pool_clone,
            task.id,
            task.current_step,
            "Resuming task",
        )
        .await
        .unwrap();
        tokio::spawn(async move {
            execute_task(pool_clone, task.id).await;
        });
    }

    Ok(())
}

async fn update_task_state(pool: &SqlitePool, task: &CompileTask) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE CompileTasks SET state = ?, progress = ?, current_step = ? WHERE id = ?",
        task.state,
        task.progress,
        task.current_step,
        task.id
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn create_task(pool: Arc<SqlitePool>, book_id: i64) -> Result<i64, AppError> {
    let now = OffsetDateTime::now_utc();
    let task = sqlx::query!(
        "INSERT INTO CompileTasks (state, progress, current_step, date_created, book_id) VALUES (?, ?, ?, ?, ?)",
        TaskState::Pending,
        0,
        CompileTaskStep::Initialize,
        now,
        book_id
    )
    .execute(&*pool)
    .await?;

    Ok(task.last_insert_rowid())
}

async fn execute_task(pool: Arc<SqlitePool>, id: i64) {
    let mut task = match sqlx::query_as!(
        CompileTask,
        r#"SELECT 
            id,
            state as "state: TaskState",
            progress,
            book_id,
            current_step as "current_step: CompileTaskStep",
            date_created as "date_created: OffsetDateTime"
            FROM CompileTasks WHERE id = ?"#,
        id
    )
    .fetch_one(&*pool)
    .await
    .ok()
    {
        Some(task) => task,
        None => return,
    };

    if task.state == TaskState::Completed {
        return;
    }

    task.state = TaskState::InProgress;
    update_task_state(&*pool, &task).await.unwrap();

    let steps = [
        CompileTaskStep::Initialize,
        CompileTaskStep::DownloadingFromSource,
        CompileTaskStep::FetchingFromSuwayomi,
        CompileTaskStep::AssemblingFile,
        CompileTaskStep::Complete,
    ];

    for step in steps.iter().skip(
        steps
            .iter()
            .position(|&s| s == task.current_step)
            .unwrap_or(0),
    ) {
        task.current_step = *step;
        update_task_state(&*pool, &task).await.unwrap();

        log_task_step(&*pool, task.id, *step, "Starting step")
            .await
            .unwrap();

        if let Err(e) = perform_step(&*pool, *step, &mut task).await {
            task.state = TaskState::Failed;
            update_task_state(&*pool, &task).await.unwrap();
            log_task_step(&*pool, task.id, *step, &format!("Step failed: {:#?}", e))
                .await
                .unwrap();
            return;
        }

        update_task_state(&*pool, &task).await.unwrap();

        log_task_step(&*pool, task.id, *step, "Step completed")
            .await
            .unwrap();
    }
}

async fn perform_step(
    pool: &SqlitePool,
    step: CompileTaskStep,
    task: &mut CompileTask,
) -> Result<(), AppError> {
    match step {
        CompileTaskStep::Initialize => {
            task.current_step = CompileTaskStep::DownloadingFromSource;
        }
        CompileTaskStep::DownloadingFromSource => {
            let book_and_chapters = match get_book_with_chapters_by_id(pool, task.book_id).await? {
                Some(book_and_chapters) => book_and_chapters,
                None => return Err(eyre!("Book not found").into()),
            };
            download_chapters_from_source(&book_and_chapters.chapters).await?;
            task.current_step = CompileTaskStep::FetchingFromSuwayomi;
        }
        CompileTaskStep::FetchingFromSuwayomi => {
            let book_and_chapters = match get_book_with_chapters_by_id(pool, task.book_id).await? {
                Some(book_and_chapters) => book_and_chapters,
                None => return Err(eyre!("Book not found").into()),
            };
            fetch_chapters_from_suwayomi(&book_and_chapters.chapters).await?;
        }
        CompileTaskStep::AssemblingFile => {
            let book_and_chapters = match get_book_with_chapters_by_id(pool, task.book_id).await? {
                Some(book_and_chapters) => book_and_chapters,
                None => return Err(eyre!("Book not found").into()),
            };
            assemble_epub_old(book_and_chapters.book, &book_and_chapters.chapters).await?;
            task.current_step = CompileTaskStep::Complete;
        }
        CompileTaskStep::Complete => {
            task.state = TaskState::Completed;
        }
    }
    Ok(())
}

pub async fn assemble_epub_old(book: Book, chapter_ids: &HashSet<i64>) -> Result<(), AppError> {
    let chapter_base_dir = &env::var("CHAPTER_DL_PATH").unwrap_or("data/chapters".to_string());
    let mut epub = EpubBuilder::new(ZipLibrary::new()?)?;
    epub.metadata("title", &book.title)?;
    epub.metadata("author", &book.author)?;

    let chapters = match get_chapters_by_ids(&chapter_ids).await? {
        Some(chapters) => chapters.nodes,
        None => return Err(eyre!("Chapters not found").into()),
    };
    // Add chapters
    for chapter in chapters {
        dbg!(&chapter);
        let chapter_dir = Path::new(&chapter_base_dir).join(&chapter.id.to_string());
        dbg!(&chapter_dir);
        let mut pages = Vec::new();

        // Read image files from the chapter directory
        for entry in fs::read_dir(chapter_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                pages.push(path);
            }
        }

        // TODO improve error handling
        pages.sort_by_key(|page_path| {
            page_path
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .parse::<i32>()
                .unwrap()
        });

        // Create chapter content
        let mut chapter_content = String::new();
        chapter_content.push_str("<?xml version='1.0' encoding='utf-8'?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\">\n<head/><body>\n");
        chapter_content.push_str(&format!("<h1>{}</h1>\n", chapter.name));

        for page in pages {
            let image_data = fs::read(&page)?;
            let og_file_name = page.file_name().unwrap().to_str().unwrap();
            let mime_type = format!("image/{}", page.extension().unwrap().to_str().unwrap());
            let file_name = format!("{}/{}", &chapter.id, og_file_name);
            epub.add_resource(&file_name, Cursor::new(image_data), &mime_type)?;
            chapter_content.push_str(&format!("<img src=\"{}\" />\n", &file_name));
        }

        chapter_content.push_str("</body>\n</html>");

        // Add chapter to EPUB
        epub.add_content(
            EpubContent::new(&chapter.name, Cursor::new(chapter_content)).title(chapter.name), // .reftype(Reference),
        )?;
    }

    let epub_base_dir = &env::var("EPUB_OUT_PATH").unwrap_or("data/epubs".to_string());
    std::fs::create_dir_all(&epub_base_dir)?;
    let epub_filename = format!("{}.epub", &book.title);
    // Generate EPUB file
    let mut output_file = File::create(Path::new(&epub_base_dir).join(epub_filename))?;
    epub.generate(&mut output_file)?;
    Ok(())
}
