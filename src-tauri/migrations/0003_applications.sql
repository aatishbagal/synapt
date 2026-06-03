CREATE TABLE IF NOT EXISTS applications (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    exec        TEXT NOT NULL,
    icon_path   TEXT,
    platform    TEXT NOT NULL,
    source_path TEXT NOT NULL UNIQUE
);

CREATE INDEX IF NOT EXISTS idx_apps_name ON applications(name);
