use serde_json::json;

use crate::ollama::tools::PersistedSourceIndex;

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
