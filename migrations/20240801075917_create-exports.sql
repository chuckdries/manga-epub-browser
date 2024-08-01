CREATE TABLE Export(
    id INTEGER PRIMARY KEY,
    title TEXT UNIQUE NOT NULL,
    author TEXT NOT NULL,
    format TEXT NOT NULL,
    state TEXT NOT NULL,
    step TEXT NOT NULL,
    progress INTEGER NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE ExportChapters(
    export_id INTEGER,
    chapter_id INTEGER,
    PRIMARY KEY (export_id, chapter_id),
    FOREIGN KEY (export_id) REFERENCES Export(id)
);

CREATE TABLE ExportLogs(
    id INTEGER PRIMARY KEY,
    export_id INTEGER NOT NULL,
    step TEXT NOT NULL,
    message TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    FOREIGN KEY (export_id) REFERENCES Export(id)
);