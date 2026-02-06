use crate::error::CheckpointSqlError;
use crate::schema::MIGRATION_STATEMENTS_SQL;
use sqlx::{Database, Pool};

pub async fn run_migrations<DB>(executor: &Pool<DB>) -> Result<(), CheckpointSqlError>
where
    DB: Database,
    for<'q> DB::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'c> &'c Pool<DB>: sqlx::Executor<'c, Database = DB>,
{
    for statement in MIGRATION_STATEMENTS_SQL {
        sqlx::query::<DB>(statement)
            .execute(executor)
            .await
            .map_err(CheckpointSqlError::Migration)?;
    }

    Ok(())
}
