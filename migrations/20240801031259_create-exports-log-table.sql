CREATE TABLE ExportLogs (
	id INTEGER PRIMARY KEY,
    -- 0: pending, 1: success, 2: failed
	status INTEGER NOT NULL,
	book_id INTEGER NOT NULL,
	file_path TEXT UNIQUE NOT NULL,
	date TEXT NOT NULL,
	FOREIGN KEY (book_id) REFERENCES Books(id)
);
