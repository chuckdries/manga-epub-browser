use std::{
    collections::HashSet,
    env,
    fs::{self, File},
    io::{self, Seek, Write},
    path::Path,
    sync::Arc,
};

use eyre::eyre;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

use crate::{models::export::{get_export_base_dir, Export}, suwayomi::get_chapters_by_ids, AppError};

#[derive(Serialize, Deserialize)]
struct CbzMetadata {
    title: String,
    author: String,
    // Add other metadata fields as needed
}

#[derive(Serialize, Deserialize)]
struct CbzChapter {
    id: i64,
    title: String,
}

// TODO log events and errors
pub async fn assemble_cbz(
    _pool: Arc<SqlitePool>,
    export: &Export,
    chapter_ids: &HashSet<i64>,
) -> Result<(), AppError> {
    let chapter_base_dir = &env::var("CHAPTER_DL_PATH").unwrap_or("data/chapters".to_string());
    let export_base_dir = &get_export_base_dir();
    std::fs::create_dir_all(&export_base_dir)?;

    let output_path = export.get_path();

    let file = File::create(&output_path)?;
    let mut zip = ZipWriter::new(file);

    let metadata = CbzMetadata {
        title: export.title.to_owned(),
        author: export.author.to_owned(),
    };

    // Write metadata
    let metadata_json = serde_json::to_string(&metadata)?;
    zip.start_file("metadata.json", SimpleFileOptions::default())?;
    zip.write_all(metadata_json.as_bytes())?;

    let chapters = match get_chapters_by_ids(&chapter_ids).await? {
        Some(chapters) => chapters.nodes,
        None => return Err(eyre!("Chapters not found").into()),
    };

    // Process chapters
    for chapter in chapters {
        let chapter_dir = Path::new(&chapter_base_dir).join(&chapter.id.to_string());
        dbg!(&chapter_dir);

        // Write chapter info
        let cbz_chapter = CbzChapter {
            id: chapter.id,
            title: chapter.name,
        };
        let chapter_info = serde_json::to_string(&cbz_chapter)?;
        zip.start_file(
            format!("{}/info.json", chapter.id),
            SimpleFileOptions::default(),
        )?;
        zip.write_all(chapter_info.as_bytes())?;

        // Add chapter images
        add_directory_to_zip(&mut zip, &chapter_dir, &chapter.id.to_string())?;
    }

    zip.finish()?;
    println!("CBZ file created: {:?}", output_path);
    Ok(())
}

fn add_directory_to_zip<T: Write + Seek>(
    zip: &mut ZipWriter<T>,
    dir_path: &Path,
    zip_path: &str,
) -> io::Result<()> {
    for entry in fs::read_dir(dir_path)? {
        dbg!(&entry);
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            println!("path is dir {:?}", path);
            add_directory_to_zip(
                zip,
                &path,
                &format!(
                    "{}/{}",
                    zip_path,
                    path.file_name().unwrap().to_str().unwrap()
                ),
            )?;
        } else {
            let mut file = File::open(&path)?;
            dbg!(&file);
            let file_name = path.file_name().unwrap().to_str().unwrap();
            zip.start_file(
                format!("{}/{}", zip_path, file_name),
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )?;
            io::copy(&mut file, zip)?;
        }
    }
    Ok(())
}
