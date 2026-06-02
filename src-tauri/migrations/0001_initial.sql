CREATE TABLE IF NOT EXISTS local_device (
    id          TEXT PRIMARY KEY DEFAULT 'self',
    device_id   TEXT NOT NULL,
    device_name TEXT NOT NULL,
    pubkey_b64  TEXT NOT NULL,
    privkey_b64 TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS trusted_peers (
    device_id   TEXT PRIMARY KEY,
    device_name TEXT NOT NULL,
    pubkey_b64  TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    paired_at   INTEGER NOT NULL,
    last_seen   INTEGER
);

CREATE TABLE IF NOT EXISTS shared_dirs (
    id   INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS indexed_dirs (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    path         TEXT NOT NULL UNIQUE,
    file_count   INTEGER DEFAULT 0,
    last_indexed INTEGER
);

CREATE TABLE IF NOT EXISTS files (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT NOT NULL,
    path          TEXT NOT NULL UNIQUE,
    parent_path   TEXT NOT NULL,
    type          TEXT NOT NULL,
    size          INTEGER,
    last_modified INTEGER,
    extension     TEXT,
    is_hidden     INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_files_name      ON files(name);
CREATE INDEX IF NOT EXISTS idx_files_parent    ON files(parent_path);
CREATE INDEX IF NOT EXISTS idx_files_extension ON files(extension);

CREATE TABLE IF NOT EXISTS transfer_history (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    peer_device_id TEXT NOT NULL,
    filename       TEXT NOT NULL,
    remote_path    TEXT NOT NULL,
    local_path     TEXT NOT NULL,
    size           INTEGER,
    bytes_received INTEGER NOT NULL DEFAULT 0,
    status         TEXT NOT NULL,
    started_at     INTEGER NOT NULL,
    completed_at   INTEGER
);

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT OR IGNORE INTO settings (key, value) VALUES
  ('max_results',    '50'),
  ('include_hidden', 'false'),
  ('theme',          'dark'),
  ('hotkey',         'ctrl+space'),
  ('download_dir',   '');
