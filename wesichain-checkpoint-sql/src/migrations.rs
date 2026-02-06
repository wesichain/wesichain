use crate::error::CheckpointSqlError;
use crate::schema::MIGRATION_STATEMENTS_SQL;

pub async fn run_migrations(executor: &sqlx::AnyPool) -> Result<(), CheckpointSqlError> {
    for statement in MIGRATION_STATEMENTS_SQL {
        sqlx::query::<sqlx::Any>(statement)
            .execute(executor)
            .await
            .map_err(CheckpointSqlError::Migration)?;
    }

    Ok(())
}
