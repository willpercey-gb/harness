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
DEFINE FIELD IF NOT EXISTS context_anchor               ON chat_session TYPE option<string>;
DEFINE FIELD IF NOT EXISTS context_priorities           ON chat_session FLEXIBLE TYPE array DEFAULT [];
DEFINE FIELD IF NOT EXISTS context_asides               ON chat_session FLEXIBLE TYPE array DEFAULT [];
DEFINE FIELD IF NOT EXISTS context_updated_at           ON chat_session TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS context_turns_since_refresh  ON chat_session TYPE int DEFAULT 0;

DEFINE TABLE IF NOT EXISTS settings SCHEMALESS;
"#;
