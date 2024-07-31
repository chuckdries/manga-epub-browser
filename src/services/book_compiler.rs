use std::{
    collections::HashSet,
    env,
    fs::{self, File},
    io::Cursor,
    path::Path,
};

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
        None => return Err(eyre!("Book not found").into()),
    };
    download_chapters_from_source(&book_and_chapters.chapters, book_id, &pool).await?;
    update_book_status(&pool, book_id, BookStatus::DownloadingFromSuwayomi).await?;
    println!("Downloading chapters from Suwayomi");
    fetch_chapters_from_suwayomi(&book_and_chapters.chapters, book_id).await?;
    update_book_status(&pool, book_id, BookStatus::Assembling).await?;
    println!("Assembling book");
    Ok(())
}

pub async fn assemble_epub(book: Book, chapter_ids: &HashSet<i64>) -> Result<(), AppError> {
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
