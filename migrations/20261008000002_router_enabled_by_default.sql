-- Boot the skill router enabled by default.
--
-- Same idiom as 20261007000001_skill_router.sql and
-- 20261008000001_skill_router_tuning.sql: idempotent, and it only adjusts the
-- value it seeded itself, so a user's customisation is never overwritten.
-- Caveat, documented on purpose: an explicit `false` that started from the
-- seeded `false` is indistinguishable from it and is flipped once, exactly as
-- the threshold migration did with `0.3 -> 0.10`. The router has never been
-- enabled in production, so the window is small.
--
-- Why: the campaign measured the closed six-domain catalog against the real
-- history, with the tuned thresholds and the morning-briefing criteria. It then
-- covers 100% of the sampled turns while saving 25.6% of the tool definitions,
-- and it always fails open. Read on every turn, so this takes effect without a
-- restart.
--
-- Absence is deliberately NOT enabled: `read_router_config` falls back to the
-- compiled default (`false`) when the key is missing, so a partially restored
-- or wiped `settings` table never starts sending messages to the classifier.

UPDATE settings SET value = 'true', updated_at = datetime('now')
WHERE key = 'ROUTER_ENABLED' AND value = 'false';
