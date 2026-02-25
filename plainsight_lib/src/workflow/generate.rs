use std::{
    collections::BTreeSet,
    fs,
    time::{Duration, Instant},
};

use tracing::{debug, info, warn};

use crate::{
    error::PlainSightError,
    memory::{self, ProjectMemory},
    ollama::{self, OllamaWrapper, Task},
    project_manager::ProjectContext,
};

use super::types::{ParsedFile, PromptProfile};

pub(crate) async fn generate_summaries(
    wrapper: &OllamaWrapper,
    manager: &ProjectContext,
    project_name: &str,
    parsed_files: &[ParsedFile],
    project_memory: &ProjectMemory,
) -> Result<(), PlainSightError> {
    info!(file_count = parsed_files.len(), "summary_phase_start");
    let mut file_summaries: Vec<(String, String)> = Vec::with_capacity(parsed_files.len());

    for parsed in parsed_files {
        info!(
            target_file = %parsed.relative_path,
            model_name = wrapper.model_name(Task::Summarize),
            "generate_file_summary"
        );

        let input = build_file_prompt_input(parsed, project_memory, PromptProfile::Standard)?;
        debug!(
            target_file = %parsed.relative_path,
            profile = "standard",
            payload_bytes = input.len(),
            "file_summary_payload"
        );
        let start = Instant::now();
        let mut used_compact = false;
        let mut summary = match wrapper.summarize(&input).await {
            Ok(summary) => summary,
            Err(err) if should_retry_compact_ollama_error(&err) => {
                warn!(
                    target_file = %parsed.relative_path,
                    error = %err,
                    "summary request failed with transient Ollama error; retrying with compact context"
                );
                used_compact = true;
                let fallback =
                    build_file_prompt_input(parsed, project_memory, PromptProfile::Compact)?;
                debug!(
                    target_file = %parsed.relative_path,
                    profile = "compact",
                    payload_bytes = fallback.len(),
                    "file_summary_payload"
                );
                wrapper
                    .summarize(&fallback)
                    .await
                    .or_else(|fallback_err| {
                        if should_retry_compact_ollama_error(&fallback_err) {
                            warn!(
                                target_file = %parsed.relative_path,
                                error = %fallback_err,
                                "summary compact retry also failed with transient Ollama error; skipping file"
                            );
                            Ok(String::new())
                        } else {
                            Err(PlainSightError::Ollama(fallback_err))
                        }
                    })?
            }
            Err(err) => return Err(PlainSightError::Ollama(err)),
        };
        if summary.is_empty() {
            continue;
        }
        if !used_compact && ollama::is_refusal_output(&summary) {
            warn!(
                target_file = %parsed.relative_path,
                "summary refusal detected; retrying with compact context"
            );
            let fallback = build_file_prompt_input(parsed, project_memory, PromptProfile::Compact)?;
            debug!(
                target_file = %parsed.relative_path,
                profile = "compact",
                payload_bytes = fallback.len(),
                "file_summary_payload"
            );
            summary = wrapper.summarize(&fallback).await.or_else(|fallback_err| {
                if should_retry_compact_ollama_error(&fallback_err) {
                    warn!(
                        target_file = %parsed.relative_path,
                        error = %fallback_err,
                        "summary refusal fallback failed with transient Ollama error; skipping file"
                    );
                    Ok(String::new())
                } else {
                    Err(PlainSightError::Ollama(fallback_err))
                }
            })?;
            if summary.is_empty() {
                continue;
            }
        }
        if ollama::is_refusal_output(&summary) {
            warn!(
                target_file = %parsed.relative_path,
                "summary refusal persisted; skipping file"
            );
            continue;
        }
        let elapsed = format_duration(start.elapsed());

        let summary_path = manager.file_summary_path(&parsed.path)?;
        fs::write(&summary_path, &summary).map_err(|e| {
            PlainSightError::io(
                format!("writing summary output '{}'", summary_path.display()),
                e,
            )
        })?;
        file_summaries.push((parsed.relative_path.clone(), summary.clone()));

        info!(
            target_file = %parsed.relative_path,
            model_name = wrapper.model_name(Task::Summarize),
            elapsed = %elapsed,
            summary_len = summary.len(),
            summary_path = %summary_path.display(),
            "file summary generated"
        );
    }

    info!(
        model_name = wrapper.model_name(Task::ProjectSummary),
        summary_path = %manager.summary_path().display(),
        "generate_project_summary"
    );

    let start = Instant::now();
    let summary_context = build_project_summary_context(&file_summaries);
    let project_summary = wrapper
        .project_summary(project_name, &summary_context)
        .await
        .map_err(PlainSightError::Ollama)?;
    let elapsed = format_duration(start.elapsed());

    let project_summary_path = manager.summary_path();
    fs::write(&project_summary_path, &project_summary).map_err(|e| {
        PlainSightError::io(
            format!(
                "writing project summary output '{}'",
                project_summary_path.display()
            ),
            e,
        )
    })?;

    info!(
        model_name = wrapper.model_name(Task::ProjectSummary),
        elapsed = %elapsed,
        summary_len = project_summary.len(),
        summary_path = %project_summary_path.display(),
        "project summary generated"
    );

    Ok(())
}

pub(crate) async fn generate_docs(
    wrapper: &OllamaWrapper,
    manager: &ProjectContext,
    project_name: &str,
    parsed_files: &[ParsedFile],
    project_memory: &ProjectMemory,
    project_index: &str,
) -> Result<(), PlainSightError> {
    info!(file_count = parsed_files.len(), "documentation_phase_start");

    for parsed in parsed_files {
        info!(
            target_file = %parsed.relative_path,
            model_name = wrapper.model_name(Task::Documentation),
            "generate_file_docs"
        );

        let input = build_file_prompt_input(parsed, project_memory, PromptProfile::Standard)?;
        debug!(
            target_file = %parsed.relative_path,
            profile = "standard",
            payload_bytes = input.len(),
            "file_docs_payload"
        );
        let start = Instant::now();
        let mut used_compact = false;
        let mut docs = match wrapper.document(&input).await {
            Ok(docs) => docs,
            Err(err) if should_retry_compact_ollama_error(&err) => {
                warn!(
                    target_file = %parsed.relative_path,
                    error = %err,
                    "docs request failed with transient Ollama error; retrying with compact context"
                );
                used_compact = true;
                let fallback =
                    build_file_prompt_input(parsed, project_memory, PromptProfile::Compact)?;
                debug!(
                    target_file = %parsed.relative_path,
                    profile = "compact",
                    payload_bytes = fallback.len(),
                    "file_docs_payload"
                );
                wrapper
                    .document(&fallback)
                    .await
                    .or_else(|fallback_err| {
                        if should_retry_compact_ollama_error(&fallback_err) {
                            warn!(
                                target_file = %parsed.relative_path,
                                error = %fallback_err,
                                "docs compact retry also failed with transient Ollama error; skipping file"
                            );
                            Ok(String::new())
                        } else {
                            Err(PlainSightError::Ollama(fallback_err))
                        }
                    })?
            }
            Err(err) => return Err(PlainSightError::Ollama(err)),
        };
        if docs.is_empty() {
            continue;
        }
        if !used_compact && ollama::is_refusal_output(&docs) {
            warn!(
                target_file = %parsed.relative_path,
                "docs refusal detected; retrying with compact context"
            );
            let fallback = build_file_prompt_input(parsed, project_memory, PromptProfile::Compact)?;
            debug!(
                target_file = %parsed.relative_path,
                profile = "compact",
                payload_bytes = fallback.len(),
                "file_docs_payload"
            );
            docs = wrapper.document(&fallback).await.or_else(|fallback_err| {
                if should_retry_compact_ollama_error(&fallback_err) {
                    warn!(
                        target_file = %parsed.relative_path,
                        error = %fallback_err,
                        "docs refusal fallback failed with transient Ollama error; skipping file"
                    );
                    Ok(String::new())
                } else {
                    Err(PlainSightError::Ollama(fallback_err))
                }
            })?;
            if docs.is_empty() {
                continue;
            }
        }
        if ollama::is_refusal_output(&docs) {
            warn!(
                target_file = %parsed.relative_path,
                "docs refusal persisted; skipping file"
            );
            continue;
        }
        let elapsed = format_duration(start.elapsed());

        let docs_path = manager.file_docs_path(&parsed.path)?;
        fs::write(&docs_path, docs).map_err(|e| {
            PlainSightError::io(format!("writing docs output '{}'", docs_path.display()), e)
        })?;

        info!(
            target_file = %parsed.relative_path,
            model_name = wrapper.model_name(Task::Documentation),
            elapsed = %elapsed,
            docs_path = %docs_path.display(),
            "file docs generated"
        );
    }

    info!(
        model_name = wrapper.model_name(Task::Architecture),
        architecture_path = %manager.architecture_path().display(),
        "generate_architecture_docs"
    );

    let start = Instant::now();
    let architecture = wrapper
        .architecture(project_name, project_index)
        .await
        .map_err(PlainSightError::Ollama)?;
    let elapsed = format_duration(start.elapsed());

    let architecture_path = manager.architecture_path();
    fs::write(&architecture_path, &architecture).map_err(|e| {
        PlainSightError::io(
            format!(
                "writing architecture output '{}'",
                architecture_path.display()
            ),
            e,
        )
    })?;

    info!(
        model_name = wrapper.model_name(Task::Architecture),
        elapsed = %elapsed,
        architecture_len = architecture.len(),
        architecture_path = %architecture_path.display(),
        "architecture docs generated"
    );

    Ok(())
}

pub(crate) async fn unload_tasks(wrapper: &OllamaWrapper, tasks: &[Task]) {
    let mut seen_models: BTreeSet<String> = BTreeSet::new();

    for task in tasks {
        let model_name = wrapper.model_name(*task).to_string();
        if !seen_models.insert(model_name.clone()) {
            continue;
        }

        info!(model_name = %model_name, "unload_model");
        match wrapper.unload_model(&model_name).await {
            Ok(()) => info!(model_name = %model_name, "model unloaded"),
            Err(err) => {
                warn!(model_name = %model_name, error = %err, "failed unloading model; continuing")
            }
        }
    }
}

fn build_file_prompt_input(
    parsed: &ParsedFile,
    project_memory: &ProjectMemory,
    profile: PromptProfile,
) -> Result<String, PlainSightError> {
    let (mut max_chunks, mut max_chunk_chars, max_file_symbols, max_file_imports) = match profile {
        PromptProfile::Standard => (12usize, 2200usize, 110usize, 90usize),
        PromptProfile::Compact => (6usize, 1200usize, 45usize, 30usize),
    };

    let relevant_memory =
        memory::get_relevant_memory_for_file(project_memory, parsed.path.to_str().unwrap_or(""));

    let memory_pressure = parsed.memory.symbols.len()
        + parsed.memory.imports.len()
        + relevant_memory.global_symbols.len()
        + relevant_memory.open_items.len()
        + relevant_memory.links.len();

    if memory_pressure > 200 {
        max_chunks = max_chunks.saturating_sub(2).max(3);
        max_chunk_chars = max_chunk_chars.saturating_sub(300).max(900);
    }
    if memory_pressure > 350 {
        max_chunks = max_chunks.saturating_sub(2).max(2);
        max_chunk_chars = max_chunk_chars.saturating_sub(200).max(700);
    }

    let mut source_index = parsed.source_index.clone();
    if source_index.chunks.len() > max_chunks {
        source_index.chunks.truncate(max_chunks);
    }
    for chunk in &mut source_index.chunks {
        if chunk.content.chars().count() > max_chunk_chars {
            let truncated: String = chunk.content.chars().take(max_chunk_chars).collect();
            chunk.content = format!("{truncated}...");
        }
    }

    let mut file_memory = parsed.memory.clone();
    if file_memory.symbols.len() > max_file_symbols {
        file_memory.symbols.truncate(max_file_symbols);
    }
    if file_memory.imports.len() > max_file_imports {
        file_memory.imports.truncate(max_file_imports);
    }
    file_memory.symbol_count = file_memory.symbols.len();
    file_memory.import_count = file_memory.imports.len();

    serde_json::to_string_pretty(&serde_json::json!({
        "path": parsed.relative_path,
        "source_index": source_index,
        "file_memory": file_memory,
        "project_memory": {
            "file_count": relevant_memory.file_count,
            "unique_symbol_count": relevant_memory.unique_symbol_count,
            "global_symbols": relevant_memory.global_symbols,
            "open_items": relevant_memory.open_items,
            "links": relevant_memory.links
        }
    }))
    .map_err(|e| PlainSightError::InvalidState(format!("serializing file prompt input: {e}")))
}

fn build_project_summary_context(file_summaries: &[(String, String)]) -> String {
    let mut out = String::from("# File Summaries\n\n");
    for (path, summary) in file_summaries {
        out.push_str("## ");
        out.push_str(path);
        out.push('\n');
        out.push_str(summary.trim());
        out.push_str("\n\n");
    }
    out
}

fn should_retry_compact_ollama_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("request timeout")
        || lower.contains("timed out")
        || lower.contains("stopping")
        || lower.contains("killed")
        || lower.contains("connection")
}

fn format_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let millis = d.subsec_millis();
    let mins = total_secs / 60;
    let secs = total_secs % 60;

    if mins > 0 {
        format!("{mins}m {secs}s {millis}ms")
    } else if secs > 0 {
        format!("{secs}s {millis}ms")
    } else {
        format!("{millis}ms")
    }
}
