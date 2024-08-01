use std::{
    collections::HashSet,
    env,
    fs::{self, File},
    io::Cursor,
    path::{Path, PathBuf},
    sync::Arc,
};

use epub_builder::{EpubBuilder, EpubContent, ZipLibrary};
use eyre::eyre;
use sqlx::SqlitePool;

use crate::{
    models::{export::{get_export_base_dir, Export}, export_log::log_export_step},
    suwayomi::get_chapters_by_ids,
    AppError,
};

pub async fn assemble_epub(
    pool: Arc<SqlitePool>,
    export: &Export,
    chapter_ids: &HashSet<i64>,
) -> Result<(), AppError> {
    let chapter_base_dir = &env::var("CHAPTER_DL_PATH").unwrap_or("data/chapters".to_string());
    let mut epub = EpubBuilder::new(ZipLibrary::new()?)?;
    epub.metadata("title", &export.title)?;
    epub.metadata("author", &export.author)?;

    let chapters = match get_chapters_by_ids(&chapter_ids).await? {
        Some(chapters) => chapters.nodes,
        None => return Err(eyre!("Chapters not found").into()),
    };
    // Add chapters
    for chapter in chapters {
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
            EpubContent::new(&chapter.name, Cursor::new(chapter_content)).title(&chapter.name),
        )?;
        log_export_step(
            &*pool,
            export.id,
            export.step,
            format!("Added chapter {} to epub", chapter.id).as_str(),
        )
        .await
        .unwrap();
    }

    let epub_base_dir = &get_export_base_dir();
    std::fs::create_dir_all(&epub_base_dir)?;
    // Generate EPUB file
    let mut output_file = File::create(export.get_path())?;
    epub.generate(&mut output_file)?;
    Ok(())
}
