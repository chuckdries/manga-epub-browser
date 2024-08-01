CREATE TABLE CompileTasks (
    id INTEGER PRIMARY KEY,
    date_created TEXT NOT NULL,
    state TEXT NOT NULL,
    progress INTEGER NOT NULL,
    current_step TEXT NOT NULL,
    book_id INTEGER NOT NULL,
    FOREIGN KEY (book_id) REFERENCES Books(id)
);

CREATE TABLE TaskLogs (
    id INTEGER PRIMARY KEY,
    task_id INTEGER NOT NULL,
    step TEXT NOT NULL,
    message TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    FOREIGN KEY (task_id) REFERENCES CompileTasks(id)
);