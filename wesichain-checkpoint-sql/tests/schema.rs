use wesichain_checkpoint_sql::error::CheckpointSqlError;
use wesichain_checkpoint_sql::schema::{
    CHECKPOINTS_TABLE, CREATE_CHECKPOINTS_TABLE_SQL, CREATE_GRAPH_TRIPLES_TABLE_SQL,
    CREATE_MESSAGES_TABLE_SQL, CREATE_SESSIONS_TABLE_SQL, GRAPH_TRIPLES_TABLE, MESSAGES_TABLE,
    SESSIONS_TABLE,
};

#[test]
fn schema_constants_include_expected_tables() {
    assert_eq!(CHECKPOINTS_TABLE, "checkpoints");
    assert_eq!(SESSIONS_TABLE, "sessions");
    assert_eq!(MESSAGES_TABLE, "messages");
    assert_eq!(GRAPH_TRIPLES_TABLE, "graph_triples");

    assert!(CREATE_CHECKPOINTS_TABLE_SQL.contains("checkpoints"));
    assert!(CREATE_SESSIONS_TABLE_SQL.contains("sessions"));
    assert!(CREATE_MESSAGES_TABLE_SQL.contains("messages"));
    assert!(CREATE_GRAPH_TRIPLES_TABLE_SQL.contains("graph_triples"));
    assert!(CREATE_CHECKPOINTS_TABLE_SQL.contains("state_json TEXT NOT NULL"));
}

#[test]
fn schema_typed_error_display_messages_are_useful() {
    let connection = CheckpointSqlError::Connection("pool timeout".to_string());
    assert_eq!(
        connection.to_string(),
        "checkpoint SQL connection error: pool timeout"
    );

    let migration = CheckpointSqlError::Migration("failed to create table".to_string());
    assert_eq!(
        migration.to_string(),
        "checkpoint SQL migration error: failed to create table"
    );

    let serialization = CheckpointSqlError::Serialization("invalid JSON".to_string());
    assert_eq!(
        serialization.to_string(),
        "checkpoint SQL serialization error: invalid JSON"
    );

    let query = CheckpointSqlError::Query("constraint violation".to_string());
    assert_eq!(
        query.to_string(),
        "checkpoint SQL query error: constraint violation"
    );

    let projection = CheckpointSqlError::Projection("mapping failed".to_string());
    assert_eq!(
        projection.to_string(),
        "checkpoint SQL projection error: mapping failed"
    );
}
