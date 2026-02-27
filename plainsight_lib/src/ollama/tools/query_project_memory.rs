use std::path::Path;

use serde_json::json;

use crate::memory::{self, ProjectMemory};

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
