pub const CHECKPOINTS_TABLE: &str = "checkpoints";
pub const SESSIONS_TABLE: &str = "sessions";
pub const MESSAGES_TABLE: &str = "messages";
pub const GRAPH_TRIPLES_TABLE: &str = "graph_triples";
pub const SCHEMA_VERSION: u32 = 1;

pub const CREATE_CHECKPOINTS_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS checkpoints (\
    thread_id TEXT NOT NULL,\
    seq INTEGER NOT NULL,\
    created_at TEXT NOT NULL,\
    node TEXT,\
    step INTEGER,\
    state_json TEXT NOT NULL,\
    PRIMARY KEY (thread_id, seq)\
)";

pub const CREATE_SESSIONS_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS sessions (\
    thread_id TEXT PRIMARY KEY,\
    session_id TEXT,\
    created_at TEXT,\
    updated_at TEXT\
)";

pub const CREATE_MESSAGES_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS messages (\
    thread_id TEXT NOT NULL,\
    seq INTEGER NOT NULL,\
    role TEXT NOT NULL,\
    content TEXT NOT NULL,\
    created_at TEXT,\
    PRIMARY KEY (thread_id, seq)\
)";

pub const CREATE_GRAPH_TRIPLES_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS graph_triples (\
    thread_id TEXT NOT NULL,\
    subject TEXT NOT NULL,\
    predicate TEXT NOT NULL,\
    object TEXT NOT NULL,\
    score REAL\
)";

pub const MIGRATION_STATEMENTS_SQL: [&str; 4] = [
    CREATE_CHECKPOINTS_TABLE_SQL,
    CREATE_SESSIONS_TABLE_SQL,
    CREATE_MESSAGES_TABLE_SQL,
    CREATE_GRAPH_TRIPLES_TABLE_SQL,
];
