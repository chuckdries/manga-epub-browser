use askama::Template;

use crate::suwayomi::chapters_by_ids::ChaptersByIdsChaptersNodes;

#[derive(Template)]
#[template(path = "components/chapter-table.html")]
pub struct ChapterTable {
    pub chapters: Vec<ChaptersByIdsChaptersNodes>,
}