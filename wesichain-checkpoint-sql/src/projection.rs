use crate::error::CheckpointSqlError;

pub trait CheckpointProjection {
    fn project(&self) -> Result<(), CheckpointSqlError>;
}
