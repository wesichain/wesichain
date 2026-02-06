use crate::error::CheckpointSqlError;
use crate::schema::MIGRATION_STATEMENTS_SQL;
use sqlx::{Database, Pool};

pub async fn run_migrations_with_connection<DB>(
    conn: &mut DB::Connection,
) -> Result<(), CheckpointSqlError>
where
    DB: Database,
    for<'q> DB::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'c> &'c mut DB::Connection: sqlx::Executor<'c, Database = DB>,
{
    for statement in MIGRATION_STATEMENTS_SQL {
        sqlx::query::<DB>(statement)
            .execute(&mut *conn)
            .await
            .map_err(CheckpointSqlError::Migration)?;
    }

    Ok(())
}

pub async fn run_migrations_in_transaction<DB>(
    tx: &mut sqlx::Transaction<'_, DB>,
) -> Result<(), CheckpointSqlError>
where
    DB: Database,
    for<'q> DB::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'c> &'c mut DB::Connection: sqlx::Executor<'c, Database = DB>,
{
    run_migrations_with_connection(tx.as_mut()).await
}

pub async fn run_migrations<DB>(executor: &Pool<DB>) -> Result<(), CheckpointSqlError>
where
    DB: Database,
    for<'q> DB::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'c> &'c mut DB::Connection: sqlx::Executor<'c, Database = DB>,
{
    let mut conn = executor
        .acquire()
        .await
        .map_err(CheckpointSqlError::Migration)?;

    run_migrations_with_connection(conn.as_mut()).await
}
