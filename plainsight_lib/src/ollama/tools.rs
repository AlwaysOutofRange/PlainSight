use std::path::Path;

use serde::Deserialize;
use serde_json::json;

use crate::memory::{self, ProjectMemory};

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

/// Load relevant memory for a specific file from a persisted project memory file.
///
/// * memory_file_path - Absolute or relative path to `.memory.json`.
/// * file_path - File path (relative to project root) to fetch relevant memory for.
/// * max_global_symbols - Optional cap for returned global symbols.
/// * max_open_items - Optional cap for returned open items.
/// * max_links - Optional cap for returned links.
#[ollama_rs::function]
pub async fn query_project_memory(
    memory_file_path: String,
    file_path: String,
    max_global_symbols: Option<usize>,
    max_open_items: Option<usize>,
    max_links: Option<usize>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    if !memory_file_path.ends_with(".memory.json") {
        return Ok(json!({
            "error": "memory_file_path must target a .memory.json file"
        })
        .to_string());
    }

    let path = Path::new(&memory_file_path);
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) => {
            return Ok(json!({
                "error": format!("failed to read memory file: {err}")
            })
            .to_string());
        }
    };

    let project_memory: ProjectMemory = match serde_json::from_str(&content) {
        Ok(memory) => memory,
        Err(err) => {
            return Ok(json!({
                "error": format!("failed to parse memory file JSON: {err}")
            })
            .to_string());
        }
    };

    let mut relevant = memory::get_relevant_memory_for_file(&project_memory, &file_path);

    if let Some(limit) = max_global_symbols {
        relevant.global_symbols.truncate(limit.min(200));
    }
    if let Some(limit) = max_open_items {
        relevant.open_items.truncate(limit.min(100));
    }
    if let Some(limit) = max_links {
        relevant.links.truncate(limit.min(200));
    }

    serde_json::to_string(&relevant)
        .or_else(|_| serde_json::to_string_pretty(&relevant))
        .map_err(|err| err.into())
}

/// Load source chunks for a specific file from persisted source index.
///
/// * source_index_file_path - Absolute or relative path to `.source_index.json`.
/// * file_path - File path (relative to project root).
/// * chunk_ids - Optional list of chunk IDs to fetch. If omitted, the first 2 chunks are returned.
/// * max_chars - Optional character cap for total returned source content.
#[ollama_rs::function]
pub async fn query_file_source(
    source_index_file_path: String,
    file_path: String,
    chunk_ids: Option<Vec<usize>>,
    max_chars: Option<usize>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    if !source_index_file_path.ends_with(".source_index.json") {
        return Ok(json!({
            "error": "source_index_file_path must target a .source_index.json file"
        })
        .to_string());
    }

    let content = match std::fs::read_to_string(&source_index_file_path) {
        Ok(content) => content,
        Err(err) => {
            return Ok(json!({
                "error": format!("failed to read source index file: {err}")
            })
            .to_string());
        }
    };

    let source_index: PersistedSourceIndex = match serde_json::from_str(&content) {
        Ok(index) => index,
        Err(err) => {
            return Ok(json!({
                "error": format!("failed to parse source index JSON: {err}")
            })
            .to_string());
        }
    };

    let Some(file) = source_index.files.iter().find(|f| f.path == file_path) else {
        return Ok(json!({
            "error": "file not found in source index",
            "file_path": file_path
        })
        .to_string());
    };

    let wanted = chunk_ids.unwrap_or_else(|| vec![0, 1]);
    let cap = max_chars.unwrap_or(3500).clamp(400, 12000);

    let mut total_chars = 0usize;
    let mut chunks_out = Vec::new();

    for chunk_id in wanted {
        let Some(chunk) = file.chunks.iter().find(|c| c.chunk_id == chunk_id) else {
            continue;
        };

        if total_chars >= cap {
            break;
        }

        let remaining = cap - total_chars;
        let mut content = chunk.content.clone();
        if content.chars().count() > remaining {
            content = content.chars().take(remaining).collect::<String>() + "...";
        }

        total_chars += content.chars().count();
        chunks_out.push(json!({
            "chunk_id": chunk.chunk_id,
            "start_line": chunk.start_line,
            "end_line": chunk.end_line,
            "content": content,
        }));
    }

    Ok(json!({
        "path": file.path,
        "language": file.language,
        "line_count": file.line_count,
        "chunk_count": file.chunk_count,
        "returned_chunk_count": chunks_out.len(),
        "returned_chars": total_chars,
        "chunks": chunks_out,
    })
    .to_string())
}
