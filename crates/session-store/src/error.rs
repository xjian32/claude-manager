use thiserror::Error;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("session not found: {0}")]
    NotFound(String),

    #[error("invalid data: {0}")]
    InvalidData(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
