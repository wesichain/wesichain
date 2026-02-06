use crate::error::CheckpointSqlError;

pub async fn save_checkpoint() -> Result<(), CheckpointSqlError> {
    Err(CheckpointSqlError::NotImplemented)
}

pub async fn load_checkpoint() -> Result<Option<serde_json::Value>, CheckpointSqlError> {
    Err(CheckpointSqlError::NotImplemented)
}
