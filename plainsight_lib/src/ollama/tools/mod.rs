mod query_file_source;
mod query_project_memory;

pub use query_file_source::query_file_source as file_source_tool;
pub use query_project_memory::query_project_memory as project_memory_tool;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct PersistedSourceChunk {
    chunk_id: usize,
    start_line: usize,
    end_line: usize,
    content: String,
}

#[derive(Debug, Deserialize)]
struct PersistedSourceFile {
    path: String,
    language: String,
    line_count: usize,
    chunk_count: usize,
    chunks: Vec<PersistedSourceChunk>,
}

#[derive(Debug, Deserialize)]
struct PersistedSourceIndex {
    files: Vec<PersistedSourceFile>,
}
