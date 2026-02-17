use crate::error::CheckpointSqlError;
use crate::schema::{ADD_CHECKPOINT_QUEUE_COLUMN_SQL, MIGRATION_STATEMENTS_SQL};
use sqlx::{Database, Pool};

fn is_duplicate_column_error(error: &sqlx::Error) -> bool {
    let Some(db_error) = error.as_database_error() else {
        return false;
    };

    let message = db_error.message().to_ascii_lowercase();
    message.contains("duplicate column") || message.contains("already exists")
}

pub async fn run_migrations_with_connection<DB>(
    conn: &mut DB::Connection,
) -> Result<(), CheckpointSqlError>
where
    DB: Database,
    for<'q> DB::Arguments<'q>: sqlx::IntoArguments<'q, DB>,
    for<'c> &'c mut DB::Connection: sqlx::Executor<'c, Database = DB>,
{
    for statement in MIGRATION_STATEMENTS_SQL {
        if let Err(error) = sqlx::query::<DB>(statement).execute(&mut *conn).await {
            if statement == ADD_CHECKPOINT_QUEUE_COLUMN_SQL && is_duplicate_column_error(&error) {
                continue;
            }
            return Err(CheckpointSqlError::Migration(error));
        }
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
