-- WebPush notifications: the browser subscriptions table, the reminder
-- notification marker and the WebPush settings.
--
-- Additive; it NEVER rebuilds a table. Each block:
--
--   1) `push_subscriptions`. One row per browser subscription. `endpoint` is
--      UNIQUE, and that constraint already creates the index — so no extra
--      index over `endpoint` is added. `user_id` exists but always defaults to
--      'default': Valet is single-user and OIDC identity is not wired here.
--
--   2) `reminders.notified_at`. Nullable; NULL means "not notified yet". It is
--      what stops the worker from re-pushing the same reminder.
--
--   3) The three WebPush settings. The VAPID pair is seeded empty — it is
--      generated on first use — and the grace window defaults to 24 hours.
--      Same "only overwrite empty/NULL" idiom as `20261010000007_rss_skill.sql`,
--      so a user customisation is preserved. "Empty" means NULL or blank after
--      TRIM, not just the empty string: the rest of the code trims before
--      deciding, so a whitespace-only seed must count as empty here too.
--
--   4) An index over `reminders(status, notified_at)`, the exact pair the
--      worker filters on every tick (`status='pending' AND notified_at IS NULL`)
--      to avoid a full scan.
--
-- The `ALTER TABLE ... ADD COLUMN` is placed LAST on purpose: it is the only
-- statement SQLite cannot re-run (there is no `IF NOT EXISTS` for a column).
-- Re-executing this body is therefore re-runnable *except* for that final
-- `ALTER`, which fails with a duplicate-column error and leaves everything
-- before it committed. In production the `Migrator` applies this file exactly
-- once, so that limitation never bites.

-- 1) The browser subscriptions table.
CREATE TABLE IF NOT EXISTS push_subscriptions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL DEFAULT 'default',
    endpoint TEXT NOT NULL UNIQUE,
    p256dh TEXT NOT NULL,
    auth TEXT NOT NULL,
    user_agent TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 2) The WebPush settings. Only seeded when missing, NULL or blank (after TRIM).
INSERT INTO settings (key, value, updated_at)
VALUES ('push_vapid_public_key', '', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value IS NULL OR trim(settings.value) = '';

INSERT INTO settings (key, value, updated_at)
VALUES ('push_vapid_private_key', '', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value IS NULL OR trim(settings.value) = '';

INSERT INTO settings (key, value, updated_at)
VALUES ('push_reminder_grace_hours', '24', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value IS NULL OR trim(settings.value) = '';

-- 3) The notification marker on reminders. Nullable; NULL = not notified.
ALTER TABLE reminders ADD COLUMN notified_at TEXT;

-- 4) Index for the worker's per-tick query. The worker selects
--    `WHERE status = 'pending' AND notified_at IS NULL` every 60 s; without
--    this index that is a full scan of `reminders` on every tick. Additive and
--    idempotent (`IF NOT EXISTS`).
CREATE INDEX IF NOT EXISTS idx_reminders_status_notified_at
    ON reminders(status, notified_at);
