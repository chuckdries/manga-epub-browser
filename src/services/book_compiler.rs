use anyhow::anyhow;
use sqlx::SqlitePool;

use crate::{
    ebook::{get_book_with_chapters_by_id, update_book_status, BookStatus},
    suwayomi::{download_chapters_from_source, fetch_chapter, fetch_chapters_from_suwayomi},
    AppError,
};

pub async fn begin_compile_book(pool: SqlitePool, book_id: i64) -> Result<(), AppError> {
    tokio::spawn(async move {
        match compile_book(&pool, book_id).await {
            Ok(_) => (),
            Err(e) => {
                update_book_status(&pool, book_id, BookStatus::Failed).await;
                eprintln!("Error compiling book: {:?}", e)
            }
        };
    });

    Ok(())
}

async fn compile_book(pool: &SqlitePool, book_id: i64) -> Result<(), AppError> {
    update_book_status(&pool, book_id, BookStatus::DownloadingFromSource).await?;
    println!("Downloading chapters from source");
    let book_and_chapters = match get_book_with_chapters_by_id(pool, book_id).await? {
        Some(book_and_chapters) => book_and_chapters,
        None => return Err(anyhow!("Book not found").into()),
    };
    download_chapters_from_source(&book_and_chapters.chapters, book_id, &pool).await?;
    update_book_status(&pool, book_id, BookStatus::DownloadingFromSuwayomi).await?;
    println!("Downloading chapters from Suwayomi");
    fetch_chapters_from_suwayomi(&book_and_chapters.chapters, book_id).await?;
    update_book_status(&pool, book_id, BookStatus::Assembling).await?;
    println!("Assembling book");
    Ok(())
}
