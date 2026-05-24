-- LLM-assisted triagem: persist per-session chat turns so the rig agent
-- can be replayed with conversational context when the deterministic
-- parser falls back to it.

CREATE TABLE triagem_chat_turns (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id  uuid NOT NULL REFERENCES triagem_sessions(id) ON DELETE CASCADE,
    role        text NOT NULL CHECK (role IN ('user', 'assistant')),
    content     text NOT NULL,
    created_at  timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX triagem_chat_turns_session_idx
    ON triagem_chat_turns (session_id, created_at);
