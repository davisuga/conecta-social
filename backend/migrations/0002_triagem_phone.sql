-- Track the chat phone for whatsapp triagem so we can resume an active
-- session before the user has provided their NIS (Q3).

ALTER TABLE triagem_sessions ADD COLUMN from_phone text;

CREATE INDEX triagem_active_by_phone_idx
    ON triagem_sessions (from_phone)
    WHERE completed_at IS NULL;
