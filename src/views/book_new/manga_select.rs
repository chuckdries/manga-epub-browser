use anyhow::Result;
use askama::Template;

use crate::{
    suwayomi::{self, get_library::MangaNodeThumbInfo},
    AppError,
};

#[derive(Template)]
#[template(path = "manga-select.html")]
pub struct MangaSelect {
    mangas: Vec<MangaNodeThumbInfo>,
}

#[axum::debug_handler]
pub async fn view_manga_select() -> Result<MangaSelect, AppError> {
    let mangas = suwayomi::get_library().await?;
    Ok(MangaSelect { mangas })
}
