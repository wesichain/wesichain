use crate::error::CheckpointSqlError;
use crate::schema::MIGRATION_STATEMENTS_SQL;

pub async fn run_migrations(executor: &sqlx::SqlitePool) -> Result<(), CheckpointSqlError> {
    for statement in MIGRATION_STATEMENTS_SQL {
        sqlx::query(statement)
            .execute(executor)
            .await
            .map_err(CheckpointSqlError::Migration)?;
    }

    Ok(())
}
