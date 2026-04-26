/// Idempotent schema applied on every connection.
pub const SCHEMA: &str = r#"
DEFINE TABLE IF NOT EXISTS chat_session SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS title         ON chat_session TYPE string;
DEFINE FIELD IF NOT EXISTS agent_id      ON chat_session TYPE string;
DEFINE FIELD IF NOT EXISTS created_at    ON chat_session TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS last_msg_at   ON chat_session TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS message_count ON chat_session TYPE int      DEFAULT 0;
DEFINE FIELD IF NOT EXISTS memory        ON chat_session TYPE object   DEFAULT {};
DEFINE FIELD IF NOT EXISTS deleted_at    ON chat_session TYPE option<datetime>;
DEFINE INDEX IF NOT EXISTS idx_agent_active ON chat_session FIELDS agent_id, deleted_at;

DEFINE TABLE IF NOT EXISTS chat_message SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS session        ON chat_message TYPE record<chat_session>;
DEFINE FIELD IF NOT EXISTS role           ON chat_message TYPE string ASSERT $value INSIDE ['user', 'assistant', 'system'];
DEFINE FIELD IF NOT EXISTS content        ON chat_message TYPE string;
DEFINE FIELD IF NOT EXISTS content_blocks ON chat_message TYPE array DEFAULT [];
DEFINE FIELD IF NOT EXISTS agent_id       ON chat_message TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at     ON chat_message TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS idx_session_time ON chat_message FIELDS session, created_at;

-- Multi-agent context window: per-session anchor/priorities/asides cards.
-- OVERWRITE (rather than IF NOT EXISTS) so older databases that defined
-- these fields without FLEXIBLE get their definitions upgraded on
-- startup. Without this, SCHEMAFULL strict type checking can reject
-- entire UPDATEs (including soft-delete) on rows whose context_*
-- arrays drifted out of spec.
DEFINE FIELD OVERWRITE context_anchor               ON chat_session TYPE option<string>;
DEFINE FIELD OVERWRITE context_priorities           ON chat_session FLEXIBLE TYPE option<array> DEFAULT [];
DEFINE FIELD OVERWRITE context_asides               ON chat_session FLEXIBLE TYPE option<array> DEFAULT [];
DEFINE FIELD OVERWRITE context_updated_at           ON chat_session TYPE option<datetime>;
DEFINE FIELD OVERWRITE context_turns_since_refresh  ON chat_session TYPE option<int> DEFAULT 0;

DEFINE TABLE IF NOT EXISTS settings SCHEMALESS;
"#;
