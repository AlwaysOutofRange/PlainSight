use std::path::PathBuf;

use crate::{memory::FileMemory, source_indexer::SourceIndex};

#[derive(Debug, Clone)]
pub(crate) struct ParsedFile {
    pub path: PathBuf,
    pub relative_path: String,
    pub language: String,
    pub hash: String,
    pub source_index: SourceIndex,
    pub memory: FileMemory,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PromptProfile {
    Standard,
    Compact,
}
