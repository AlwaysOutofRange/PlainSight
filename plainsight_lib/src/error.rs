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

    #[error("parse error: {0}")]
    Parse(String),

    #[error("failed to load query file '{path}': {source}")]
    QueryLoad {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to compile '{kind}' query: {detail}")]
    QueryCompile { kind: String, detail: String },

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
