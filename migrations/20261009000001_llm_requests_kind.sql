-- Add the origin discriminator to `llm_requests`.
--
-- Five processes share the table (chat, router, archivist, consolidator,
-- collapse). Before this migration they were indistinguishable except by
-- model, which polluted the chat aggregates with background pipeline work.
-- Existing rows inherit the default `'chat'`.
ALTER TABLE llm_requests
    ADD COLUMN kind TEXT NOT NULL DEFAULT 'chat'
    CHECK (kind IN ('chat', 'router', 'archivist', 'consolidator', 'collapse'));
