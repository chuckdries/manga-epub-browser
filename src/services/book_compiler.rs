use std::{collections::HashSet, env, fs::{self, File}, path::Path};

use anyhow::{anyhow, Error};
use epub_builder::{EpubBuilder, EpubContent, ZipLibrary};
use eyre::eyre;
use sqlx::{any, SqlitePool};

use crate::{
    ebook::{get_book_with_chapters_by_id, update_book_status, Book, BookStatus},
    suwayomi::{
        download_chapters_from_source, fetch_chapter, fetch_chapters_from_suwayomi,
        get_chapters_by_ids,
    },
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

async fn assemble_epub(book: Book, chapter_ids: &HashSet<i64>) -> Result<(), AppError> {
    let base_dir = &env::var("CHAPTER_DL_PATH").unwrap_or("data/chapters".to_string());
    let mut epub = EpubBuilder::new(ZipLibrary::new()?)?;
    epub.metadata("title", book.title)?;
    epub.metadata("author", book.author)?;

    let chapters = match get_chapters_by_ids(&chapter_ids).await? {
        Some(chapters) => chapters.nodes,
        None => return Err(anyhow!("Chapters not found").into()),
    };
    // Add chapters
    for chapter in chapters {
        let chapter_dir = Path::new(&base_dir).join(&chapter.id.to_string());
        let mut pages = Vec::new();

        // Read image files from the chapter directory
        for entry in fs::read_dir(chapter_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                pages.push(path);
            }
        }

        // Sort pages alphabetically
        pages.sort();

        // Create chapter content
        let mut chapter_content = String::new();
        chapter_content.push_str(&format!("<h1>{}</h1>", chapter.name));

        for page in pages {
            let image_data = fs::read(&page)?;
            let file_name = page.file_name().unwrap().to_str().unwrap();
            epub.add_resource(file_name, image_data, "image/jpeg")?;
            chapter_content.push_str(&format!("<img src=\"{}\" />", file_name));
        }

        // Add chapter to EPUB
        epub.add_content(
            EpubContent::new(chapter.name, chapter_content)
                .title(chapter.name)
                .reftype("chapter"),
        )?;
    }

    // Generate EPUB file
    let mut output_file = File::create("output.epub")?;
    epub.generate(&mut output_file)?;
    Ok(())
}
