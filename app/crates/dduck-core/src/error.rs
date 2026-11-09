//! Error type. Messages never include transcript content (D-021): only paths, line numbers and categories.

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("invalid configuration: {0}")]
    Config(&'static str),
}
