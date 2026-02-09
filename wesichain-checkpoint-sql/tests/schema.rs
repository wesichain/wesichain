use sqlx::Connection;
use wesichain_checkpoint_sql::error::CheckpointSqlError;
use wesichain_checkpoint_sql::schema::{
    CHECKPOINTS_TABLE, CREATE_CHECKPOINTS_TABLE_SQL, CREATE_GRAPH_TRIPLES_TABLE_SQL,
    CREATE_MESSAGES_TABLE_SQL, CREATE_SESSIONS_TABLE_SQL, GRAPH_TRIPLES_TABLE, MESSAGES_TABLE,
    MIGRATION_STATEMENTS_SQL, SCHEMA_VERSION, SESSIONS_TABLE,
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

    assert_eq!(SCHEMA_VERSION, 1);
    assert_eq!(MIGRATION_STATEMENTS_SQL.len(), 4);
    assert_eq!(MIGRATION_STATEMENTS_SQL[0], CREATE_CHECKPOINTS_TABLE_SQL);
    assert_eq!(MIGRATION_STATEMENTS_SQL[1], CREATE_SESSIONS_TABLE_SQL);
    assert_eq!(MIGRATION_STATEMENTS_SQL[2], CREATE_MESSAGES_TABLE_SQL);
    assert_eq!(MIGRATION_STATEMENTS_SQL[3], CREATE_GRAPH_TRIPLES_TABLE_SQL);
}

#[test]
fn schema_uses_bigint_for_sequence_columns() {
    assert!(
        CREATE_CHECKPOINTS_TABLE_SQL.contains("seq BIGINT NOT NULL"),
        "checkpoints.seq should be BIGINT for postgres/sqlx i64 compatibility"
    );
    assert!(
        CREATE_CHECKPOINTS_TABLE_SQL.contains("step BIGINT"),
        "checkpoints.step should be BIGINT for postgres/sqlx i64 compatibility"
    );
    assert!(
        CREATE_MESSAGES_TABLE_SQL.contains("seq BIGINT NOT NULL"),
        "messages.seq should be BIGINT for postgres/sqlx i64 compatibility"
    );
}

#[test]
fn schema_typed_error_display_messages_are_useful() {
    let connection = CheckpointSqlError::Connection(sqlx::Error::RowNotFound);
    assert!(connection
        .to_string()
        .contains("checkpoint SQL connection error"));

    let migration = CheckpointSqlError::Migration(sqlx::Error::RowNotFound);
    assert!(migration
        .to_string()
        .contains("checkpoint SQL migration error"));

    let serialization_source = serde_json::from_str::<serde_json::Value>("not json")
        .expect_err("invalid JSON should produce serde_json::Error");
    let serialization = CheckpointSqlError::Serialization(serialization_source);
    assert!(serialization
        .to_string()
        .contains("checkpoint SQL serialization error"));

    let query = CheckpointSqlError::Query(sqlx::Error::RowNotFound);
    assert!(query.to_string().contains("checkpoint SQL query error"));

    let projection = CheckpointSqlError::Projection("mapping failed".to_owned());
    assert_eq!(
        projection.to_string(),
        "checkpoint SQL projection error: mapping failed"
    );
}

#[test]
fn schema_errors_preserve_typed_sources() {
    let sqlx_source = sqlx::Error::RowNotFound;
    let query = CheckpointSqlError::Query(sqlx_source);
    match query {
        CheckpointSqlError::Query(sqlx::Error::RowNotFound) => {}
        _ => panic!("expected sqlx::Error::RowNotFound source"),
    }

    let serde_source =
        serde_json::from_str::<serde_json::Value>("not json").expect_err("invalid json");
    let serialization = CheckpointSqlError::Serialization(serde_source);
    match serialization {
        CheckpointSqlError::Serialization(source) => {
            assert!(source.is_syntax());
        }
        _ => panic!("expected serde_json::Error source"),
    }
}

#[tokio::test]
async fn schema_migrations_execute_on_in_memory_sqlite() {
    let mut connection = sqlx::SqliteConnection::connect(":memory:")
        .await
        .expect("in-memory sqlite connection should open");

    for statement in MIGRATION_STATEMENTS_SQL {
        sqlx::query(statement)
            .execute(&mut connection)
            .await
            .expect("migration statement should execute");
    }
}
