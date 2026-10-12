-- Notification ingest: the notification inbox table and the application keys
-- that authorize `POST /api/notifications` without an OIDC session.
--
-- Additive and fully idempotent: every statement uses `IF NOT EXISTS`, so the
-- whole file is re-runnable as-is. It never rebuilds a table and adds no column
-- to an existing one — hence there is NO trailing `ALTER TABLE`, the one
-- statement SQLite cannot re-run. Each block:
--
--   1) `notifications`. One row per ingested notification. `metadata` is a
--      serialized JSON object or NULL; `read_at` is NULL until the notification
--      is marked read; `level` is a whitelisted severity defaulting to 'info'.
--
--   2) `ingest_keys`. One row per application key. Only the SHA-256 of the token
--      (`key_hash`) and its first 12 characters (`prefix`) are stored — never
--      the token itself. `name` and `key_hash` are UNIQUE, and those constraints
--      already create their indexes, so no extra index is added over them.
--      `revoked_at` marks a key as no longer usable without deleting the row.
--
--   3) A partial index over `notifications(created_at) WHERE read_at IS NULL`,
--      matching the unread query, which filters on `read_at IS NULL` and orders
--      by `created_at DESC`. Indexing `created_at` (not `read_at`) is what makes
--      the ordering cheap; the partial clause keeps the index tiny.
--
--   4) An index over `notifications(origin, created_at)` for filtering by origin.

-- 1) The notification inbox.
CREATE TABLE IF NOT EXISTS notifications (
    id TEXT PRIMARY KEY,
    origin TEXT NOT NULL,
    author TEXT NOT NULL,
    message TEXT NOT NULL,
    level TEXT NOT NULL DEFAULT 'info',
    metadata TEXT,
    read_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 2) The ingest application keys. `name` and `key_hash` are UNIQUE; those
--    constraints create their indexes implicitly, so none is added over them.
CREATE TABLE IF NOT EXISTS ingest_keys (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    key_hash TEXT NOT NULL UNIQUE,
    prefix TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_used_at TEXT,
    revoked_at TEXT
);

-- 3) Partial index for the unread query (`WHERE read_at IS NULL ORDER BY
--    created_at DESC`). Additive and idempotent.
CREATE INDEX IF NOT EXISTS idx_notifications_unread
    ON notifications(created_at) WHERE read_at IS NULL;

-- 4) Index for filtering notifications by origin.
CREATE INDEX IF NOT EXISTS idx_notifications_origin
    ON notifications(origin, created_at);
