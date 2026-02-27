use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlainSightError {
    #[error("I/O error while {context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },

    #[error("ollama error: {0}")]
    Ollama(String),

    #[error("file path '{path}' is outside project root '{project_root}'")]
    PathOutsideProject {
        path: PathBuf,
        project_root: PathBuf,
    },

    #[error("invalid state: {0}")]
    InvalidState(String),
}

impl PlainSightError {
    pub fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }
}

pub type Result<T> = std::result::Result<T, PlainSightError>;
