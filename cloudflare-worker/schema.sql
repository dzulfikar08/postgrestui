-- D1 schema for PostgresTUI releases
-- Run: wrangler d1 execute postgrestui-releases-db --remote --file=./schema.sql

CREATE TABLE IF NOT EXISTS release_files (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  version INTEGER NOT NULL,
  platform TEXT NOT NULL,
  filename TEXT NOT NULL,
  r2_key TEXT NOT NULL,
  size INTEGER NOT NULL,
  created_at TEXT DEFAULT (datetime('now'))
);
