-- Key/value app settings (e.g. global reading direction).
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Registered libraries: a local folder or a folder on an SSH host.
CREATE TABLE IF NOT EXISTS libraries (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    kind      TEXT NOT NULL CHECK (kind IN ('local', 'sftp')),
    name      TEXT NOT NULL,
    host      TEXT,                -- ~/.ssh/config Host alias for sftp libraries
    root      TEXT NOT NULL,       -- absolute folder path (local or remote)
    direction TEXT CHECK (direction IN ('rtl', 'ltr'))
);

-- Per-work (folder) reading direction override.
CREATE TABLE IF NOT EXISTS folder_settings (
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    work_id    TEXT NOT NULL,
    direction  TEXT CHECK (direction IN ('rtl', 'ltr')),
    PRIMARY KEY (library_id, work_id)
);

-- Per-volume reading state. volume_key is "<zip rel path>" or "<zip rel path>#<inner dir>".
CREATE TABLE IF NOT EXISTS book_state (
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    volume_key TEXT NOT NULL,
    page       INTEGER NOT NULL DEFAULT 0,
    page_count INTEGER NOT NULL DEFAULT 0,
    shift      INTEGER NOT NULL DEFAULT 0,
    direction  TEXT CHECK (direction IN ('rtl', 'ltr')),
    updated_at INTEGER NOT NULL DEFAULT 0, -- unix seconds of the last page save, 0 if never read
    PRIMARY KEY (library_id, volume_key)
);

-- Cached zip indexes, valid while size and mtime match the file.
CREATE TABLE IF NOT EXISTS zip_cache (
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    rel_path   TEXT NOT NULL,
    size       INTEGER NOT NULL,
    mtime      INTEGER NOT NULL,
    index_json TEXT NOT NULL,
    PRIMARY KEY (library_id, rel_path)
);

-- Last scan result per library, shown immediately while a rescan runs.
CREATE TABLE IF NOT EXISTS works_cache (
    library_id INTEGER PRIMARY KEY REFERENCES libraries(id) ON DELETE CASCADE,
    works_json TEXT NOT NULL
);
