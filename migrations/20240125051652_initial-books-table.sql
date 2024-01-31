CREATE TABLE Books(
  id INTEGER PRIMARY KEY,
  manga_id INTEGER,
  title STRING NOT NULL,
  author STRING NOT NULL,
  -- 1: draft
  status INTEGER NOT NULL
);

CREATE TABLE BookChapters(
  book_id INTEGER,
  chapter_id INTEGER,
  PRIMARY KEY (book_id, chapter_id),
  FOREIGN KEY (book_id) REFERENCES Books(id)
);