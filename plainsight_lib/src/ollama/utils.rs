use serde_json::{Value, json};

use super::Task;

pub fn ensure_non_empty(task: Task, model_name: &str, output: String) -> Result<String, String> {
    if output.trim().is_empty() {
        return Err(format!(
            "ollama returned empty output for task {:?} ({})",
            task, model_name
        ));
    }
    Ok(output)
}

pub fn is_refusal_output(output: &str) -> bool {
    let lower = output.to_lowercase();
    lower.contains("i cannot")
        || lower.contains("i can't")
        || lower.contains("i'm unable")
        || lower.contains("as an ai")
        || lower.contains("i don't have")
        || lower.contains("i do not have")
        || lower.contains("i am not able")
        || lower.contains("unable to")
        || lower.contains("cannot help")
        || lower.contains("can't help")
        || lower.contains("not allowed")
        || lower.contains("not permitted")
        || lower.contains("against my")
        || lower.contains("ethical")
        || lower.contains("policy")
        || lower.contains("guidelines")
}

pub fn strip_wrapping_code_fence(output: String) -> String {
    let trimmed = output.trim();
    if trimmed.starts_with("```") && trimmed.ends_with("```") {
        let without_fences = trimmed.trim_start_matches("```").trim_end_matches("```");
        let lines: Vec<&str> = without_fences.lines().collect();
        // Remove language specifier if present on first line
        if !lines.is_empty() && lines[0].trim().chars().all(|c| c.is_alphabetic()) {
            lines[1..].join("\n").trim().to_string()
        } else {
            lines.join("\n").trim().to_string()
        }
    } else {
        trimmed.to_string()
    }
}

pub fn unwrap_json_markdown(task: Task, output: String) -> String {
    let trimmed = output.trim();
    let parsed: Value = match serde_json::from_str(trimmed) {
        Ok(value) => value,
        Err(_) => return output,
    };

    if let Some(text) = parsed
        .pointer("/result/summary_markdown")
        .and_then(Value::as_str)
    {
        return text.trim().to_string();
    }
    if let Some(text) = parsed
        .pointer("/result/docs_markdown")
        .and_then(Value::as_str)
    {
        return text.trim().to_string();
    }
    if let Some(text) = parsed
        .pointer("/result/project_summary_markdown")
        .and_then(Value::as_str)
    {
        return text.trim().to_string();
    }
    if let Some(text) = parsed
        .pointer("/result/architecture_markdown")
        .and_then(Value::as_str)
    {
        return text.trim().to_string();
    }
    if let Some(text) = parsed.get("summary_markdown").and_then(Value::as_str) {
        return text.trim().to_string();
    }
    if let Some(text) = parsed.get("docs_markdown").and_then(Value::as_str) {
        return text.trim().to_string();
    }

    let expected_headings = expected_headings(task);
    if let Some(text) = find_markdown_string(&parsed, expected_headings) {
        return text.trim().to_string();
    }

    output
}

pub fn trim_to_expected_heading(task: Task, output: String) -> String {
    let expected = expected_headings(task);

    for heading in expected {
        if let Some(idx) = output.find(heading) {
            return output[idx..].trim().to_string();
        }
    }

    output.trim().to_string()
}

pub fn reject_json_payload(output: String) -> Result<String, String> {
    let trimmed = output.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return Err("ollama returned JSON payload instead of markdown".to_string());
    }
    Ok(output)
}

fn expected_headings(task: Task) -> &'static [&'static str] {
    match task {
        Task::Summarize => &["## Purpose"],
        Task::Documentation => &["## Overview"],
        Task::ProjectSummary => &["## Overview"],
        Task::Architecture => &["## System Context"],
    }
}

fn find_markdown_string(value: &Value, expected_headings: &[&str]) -> Option<String> {
    match value {
        Value::String(s) => {
            if expected_headings.iter().any(|heading| s.contains(heading)) || s.contains("## ") {
                Some(s.clone())
            } else {
                None
            }
        }
        Value::Array(items) => {
            for item in items {
                if let Some(found) = find_markdown_string(item, expected_headings) {
                    return Some(found);
                }
            }
            None
        }
        Value::Object(map) => {
            for item in map.values() {
                if let Some(found) = find_markdown_string(item, expected_headings) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

pub fn ensure_ai_disclaimer(output: String) -> String {
    let disclaimer = "> **AI-generated content:** May contain inaccuracies. Verify against source code.";
    let trimmed = output.trim();

    let lower = trimmed.to_lowercase();
    if lower.starts_with("> **ai-generated content:**")
        || lower.starts_with("**ai-generated content:**")
        || lower.starts_with("<!-- generated by ai")
    {
        return output;
    }

    if trimmed.starts_with(disclaimer) {
        return output;
    }

    if trimmed.is_empty() {
        disclaimer.to_string()
    } else {
        format!("{}\n\n{}", disclaimer, trimmed)
    }
}

pub fn prepare_file_summary_input(context_payload: &str) -> Result<String, String> {
    let mut v: Value = serde_json::from_str(context_payload).map_err(|e| e.to_string())?;
    clamp_chunks_in_payload(&mut v, 4, 900);
    clamp_global_symbols(&mut v, 60);
    clamp_open_items(&mut v, 24);
    clamp_links(&mut v, 40);
    serde_json::to_string(&v).map_err(|e| e.to_string())
}

pub fn prepare_file_docs_input(context_payload: &str) -> Result<String, String> {
    let mut v: Value = serde_json::from_str(context_payload).map_err(|e| e.to_string())?;
    clamp_chunks_in_payload(&mut v, 6, 1200);
    clamp_global_symbols(&mut v, 80);
    clamp_open_items(&mut v, 30);
    clamp_links(&mut v, 70);
    serde_json::to_string(&v).map_err(|e| e.to_string())
}

pub fn prepare_architecture_input(context_payload: &str) -> Result<String, String> {
    build_project_digest(context_payload, true)
}

fn build_project_digest(
    context_payload: &str,
    include_chunk_preview: bool,
) -> Result<String, String> {
    let v: Value = serde_json::from_str(context_payload).map_err(|e| e.to_string())?;
    let files = v
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| "project index input missing 'files' array".to_string())?;

    let mut file_entries = Vec::with_capacity(files.len());
    for file in files {
        let path = file
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let symbols = file.get("symbols").cloned().unwrap_or(Value::Null);

        let line_count = symbols
            .get("line_count")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let chunk_count = symbols
            .get("chunk_count")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let preview = chunk_preview(&symbols, 200);

        let mut entry = json!({
            "path": path,
            "line_count": line_count,
            "chunk_count": chunk_count,
        });

        if include_chunk_preview {
            entry["preview"] = json!(preview);
        }

        file_entries.push(entry);
    }

    let summary = json!({
        "project": v.get("project").cloned().unwrap_or(json!("unknown")),
        "file_count": v.get("file_count").cloned().unwrap_or(json!(file_entries.len())),
        "files": file_entries
    });

    serde_json::to_string(&summary).map_err(|e| e.to_string())
}

fn chunk_preview(root: &Value, max_chars: usize) -> String {
    let chunks = root
        .get("chunks")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .cloned()
        .unwrap_or(Value::Null);
    let content = chunks
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();

    if content.chars().count() <= max_chars {
        return content.to_string();
    }

    let truncated: String = content.chars().take(max_chars).collect();
    format!("{truncated}...")
}

fn clamp_chunks_in_payload(root: &mut Value, max_chunks: usize, max_chars_per_chunk: usize) {
    let source_index = if root.get("source_index").is_some_and(Value::is_object) {
        match root.get_mut("source_index") {
            Some(value) => value,
            None => root,
        }
    } else {
        root
    };
    let Some(chunks) = source_index.get_mut("chunks").and_then(Value::as_array_mut) else {
        return;
    };

    if chunks.len() > max_chunks {
        chunks.truncate(max_chunks);
    }

    for chunk in chunks {
        if let Some(Value::String(content)) = chunk.get_mut("content") {
            if content.chars().count() > max_chars_per_chunk {
                let truncated: String = content.chars().take(max_chars_per_chunk).collect();
                *content = format!("{truncated}...");
            }
        }
    }
}

fn clamp_global_symbols(root: &mut Value, max_symbols: usize) {
    let Some(symbols) = root
        .get_mut("project_memory")
        .and_then(Value::as_object_mut)
        .and_then(|obj| obj.get_mut("global_symbols"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };

    if symbols.len() > max_symbols {
        symbols.truncate(max_symbols);
    }
}

fn clamp_open_items(root: &mut Value, max_items: usize) {
    let Some(items) = root
        .get_mut("project_memory")
        .and_then(Value::as_object_mut)
        .and_then(|obj| obj.get_mut("open_items"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };

    if items.len() > max_items {
        items.truncate(max_items);
    }
}

fn clamp_links(root: &mut Value, max_links: usize) {
    let Some(links) = root
        .get_mut("project_memory")
        .and_then(Value::as_object_mut)
        .and_then(|obj| obj.get_mut("links"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };

    if links.len() > max_links {
        links.truncate(max_links);
    }
}
