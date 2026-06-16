use thiserror::Error;

#[derive(Debug, Error)]
pub enum AshkorixError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("model error: {0}")]
    Model(String),

    #[error("import error: {0}")]
    Import(String),

    #[error("index error: {0}")]
    Index(String),

    #[error("retrieval error: {0}")]
    Retrieval(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("cancelled")]
    Cancelled,

    #[error("extension error: {0}")]
    Extension(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AshkorixError>;
