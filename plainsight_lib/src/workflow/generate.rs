use std::{
    collections::BTreeSet,
    fs,
    path::Path,
    time::{Duration, Instant},
};

use tracing::{debug, info, warn};

use crate::{
    error::{PlainSightError, Result as PlainResult},
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
    memory_file_path: &Path,
    source_index_file_path: &Path,
    files_to_regenerate: &BTreeSet<String>,
) -> PlainResult<()> {
    info!(file_count = parsed_files.len(), "summary_phase_start");
    let mut file_summaries: Vec<(String, String)> = Vec::with_capacity(parsed_files.len());
    let mut summary_reused = 0usize;
    let mut summary_generated = 0usize;
    let mut summary_skipped = 0usize;

    for parsed in parsed_files {
        if !files_to_regenerate.contains(&parsed.relative_path) {
            let summary_path = manager.file_summary_path(&parsed.path)?;
            if let Ok(existing_summary) = fs::read_to_string(&summary_path) {
                if !existing_summary.trim().is_empty() {
                    file_summaries.push((parsed.relative_path.clone(), existing_summary));
                    summary_reused += 1;
                    debug!(
                        target_file = %parsed.relative_path,
                        summary_path = %summary_path.display(),
                        "reuse_file_summary"
                    );
                    continue;
                }
            }
        }

        debug!(
            target_file = %parsed.relative_path,
            model_name = wrapper.model_name(Task::Summarize),
            "generate_file_summary"
        );

        debug_current_memory(memory_file_path, &parsed.relative_path);

        let input = build_file_prompt_input(
            parsed,
            project_memory,
            PromptProfile::Standard,
            memory_file_path,
            source_index_file_path,
        )?;
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
                let fallback = build_file_prompt_input(
                    parsed,
                    project_memory,
                    PromptProfile::Compact,
                    memory_file_path,
                    source_index_file_path,
                )?;
                debug!(
                    target_file = %parsed.relative_path,
                    profile = "compact",
                    payload_bytes = fallback.len(),
                    "file_summary_payload"
                );
                wrapper.summarize(&fallback).await.or_else(|fallback_err| {
                    if should_retry_compact_ollama_error(&fallback_err) {
                        warn!(
                            target_file = %parsed.relative_path,
                            error = %fallback_err,
                            "summary compact retry also failed with transient Ollama error; skipping file"
                        );
                        Ok(String::new())
                    } else {
                        Err(fallback_err)
                    }
                })?
            }
            Err(err) => return Err(err),
        };

        if summary.is_empty() {
            summary_skipped += 1;
            continue;
        }

        if !used_compact && ollama::is_refusal_output(&summary) {
            warn!(
                target_file = %parsed.relative_path,
                "summary refusal detected; retrying with compact context"
            );
            let fallback = build_file_prompt_input(
                parsed,
                project_memory,
                PromptProfile::Compact,
                memory_file_path,
                source_index_file_path,
            )?;
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
                    Err(fallback_err)
                }
            })?;
            if summary.is_empty() {
                summary_skipped += 1;
                continue;
            }
        }

        if ollama::is_refusal_output(&summary) {
            warn!(
                target_file = %parsed.relative_path,
                "summary refusal persisted; skipping file"
            );
            summary_skipped += 1;
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

        // Keep memory snapshot fresh for each generated artifact.
        sync_memory_snapshot(memory_file_path, project_memory, "after_file_summary")?;

        file_summaries.push((parsed.relative_path.clone(), summary.clone()));
        summary_generated += 1;

        debug!(
            target_file = %parsed.relative_path,
            model_name = wrapper.model_name(Task::Summarize),
            elapsed = %elapsed,
            summary_len = summary.len(),
            summary_path = %summary_path.display(),
            "file summary generated"
        );
    }

    if files_to_regenerate.is_empty() {
        info!("project_summary_unchanged_skip");
        info!(
            reused = summary_reused,
            generated = summary_generated,
            skipped = summary_skipped,
            "summary_phase_complete"
        );
        return Ok(());
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
        .await?;
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
    sync_memory_snapshot(memory_file_path, project_memory, "after_project_summary")?;

    info!(
        model_name = wrapper.model_name(Task::ProjectSummary),
        elapsed = %elapsed,
        summary_len = project_summary.len(),
        summary_path = %project_summary_path.display(),
        "project summary generated"
    );
    info!(
        reused = summary_reused,
        generated = summary_generated,
        skipped = summary_skipped,
        "summary_phase_complete"
    );

    Ok(())
}

pub(crate) async fn generate_docs(
    wrapper: &OllamaWrapper,
    manager: &ProjectContext,
    project_name: &str,
    parsed_files: &[ParsedFile],
    project_memory: &ProjectMemory,
    memory_file_path: &Path,
    source_index_file_path: &Path,
    project_index: &str,
    files_to_regenerate: &BTreeSet<String>,
) -> PlainResult<()> {
    info!(file_count = parsed_files.len(), "documentation_phase_start");
    let mut docs_reused = 0usize;
    let mut docs_generated = 0usize;
    let mut docs_skipped = 0usize;

    for parsed in parsed_files {
        if !files_to_regenerate.contains(&parsed.relative_path) {
            docs_reused += 1;
            debug!(target_file = %parsed.relative_path, "reuse_file_docs");
            continue;
        }

        debug!(
            target_file = %parsed.relative_path,
            model_name = wrapper.model_name(Task::Documentation),
            "generate_file_docs"
        );

        debug_current_memory(memory_file_path, &parsed.relative_path);

        let input = build_file_prompt_input(
            parsed,
            project_memory,
            PromptProfile::Standard,
            memory_file_path,
            source_index_file_path,
        )?;
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
                let fallback = build_file_prompt_input(
                    parsed,
                    project_memory,
                    PromptProfile::Compact,
                    memory_file_path,
                    source_index_file_path,
                )?;
                debug!(
                    target_file = %parsed.relative_path,
                    profile = "compact",
                    payload_bytes = fallback.len(),
                    "file_docs_payload"
                );
                wrapper.document(&fallback).await.or_else(|fallback_err| {
                    if should_retry_compact_ollama_error(&fallback_err) {
                        warn!(
                            target_file = %parsed.relative_path,
                            error = %fallback_err,
                            "docs compact retry also failed with transient Ollama error; skipping file"
                        );
                        Ok(String::new())
                    } else {
                        Err(fallback_err)
                    }
                })?
            }
            Err(err) => return Err(err),
        };

        if docs.is_empty() {
            docs_skipped += 1;
            continue;
        }

        if !used_compact && ollama::is_refusal_output(&docs) {
            warn!(
                target_file = %parsed.relative_path,
                "docs refusal detected; retrying with compact context"
            );
            let fallback = build_file_prompt_input(
                parsed,
                project_memory,
                PromptProfile::Compact,
                memory_file_path,
                source_index_file_path,
            )?;
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
                    Err(fallback_err)
                }
            })?;
            if docs.is_empty() {
                docs_skipped += 1;
                continue;
            }
        }

        if ollama::is_refusal_output(&docs) {
            warn!(
                target_file = %parsed.relative_path,
                "docs refusal persisted; skipping file"
            );
            docs_skipped += 1;
            continue;
        }

        let elapsed = format_duration(start.elapsed());
        let docs_path = manager.file_docs_path(&parsed.path)?;
        fs::write(&docs_path, docs).map_err(|e| {
            PlainSightError::io(format!("writing docs output '{}'", docs_path.display()), e)
        })?;
        sync_memory_snapshot(memory_file_path, project_memory, "after_file_docs")?;

        docs_generated += 1;
        debug!(
            target_file = %parsed.relative_path,
            model_name = wrapper.model_name(Task::Documentation),
            elapsed = %elapsed,
            docs_path = %docs_path.display(),
            "file docs generated"
        );
    }

    if files_to_regenerate.is_empty() {
        info!("architecture_unchanged_skip");
        info!(
            reused = docs_reused,
            generated = docs_generated,
            skipped = docs_skipped,
            "documentation_phase_complete"
        );
        return Ok(());
    }

    info!(
        model_name = wrapper.model_name(Task::Architecture),
        architecture_path = %manager.architecture_path().display(),
        "generate_architecture_docs"
    );

    let start = Instant::now();
    let architecture = wrapper.architecture(project_name, project_index).await?;
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
    sync_memory_snapshot(memory_file_path, project_memory, "after_architecture")?;

    info!(
        model_name = wrapper.model_name(Task::Architecture),
        elapsed = %elapsed,
        architecture_len = architecture.len(),
        architecture_path = %architecture_path.display(),
        "architecture docs generated"
    );
    info!(
        reused = docs_reused,
        generated = docs_generated,
        skipped = docs_skipped,
        "documentation_phase_complete"
    );

    Ok(())
}

pub(crate) async fn unload_tasks(wrapper: &OllamaWrapper, tasks: &[Task]) {
    let mut seen_models: BTreeSet<String> = BTreeSet::new();
    let mut unload_ok = 0usize;
    let mut unload_failed = 0usize;

    for task in tasks {
        let model_name = wrapper.model_name(*task).to_string();
        if !seen_models.insert(model_name.clone()) {
            continue;
        }

        debug!(model_name = %model_name, "unload_model");
        match wrapper.unload_model(&model_name).await {
            Ok(()) => {
                unload_ok += 1;
                debug!(model_name = %model_name, "model unloaded")
            }
            Err(err) => {
                unload_failed += 1;
                warn!(model_name = %model_name, error = %err, "failed unloading model; continuing")
            }
        }
    }

    info!(
        requested_models = seen_models.len(),
        unloaded = unload_ok,
        failed = unload_failed,
        "unload_phase_complete"
    );
}

fn build_file_prompt_input(
    parsed: &ParsedFile,
    project_memory: &ProjectMemory,
    profile: PromptProfile,
    memory_file_path: &Path,
    source_index_file_path: &Path,
) -> PlainResult<String> {
    let (mut max_chunks, mut max_chunk_chars, max_file_symbols, max_file_imports) = match profile {
        PromptProfile::Standard => (8usize, 1600usize, 70usize, 50usize),
        PromptProfile::Compact => (4usize, 900usize, 30usize, 20usize),
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
        max_chunk_chars = max_chunk_chars.saturating_sub(250).max(800);
    }
    if memory_pressure > 350 {
        max_chunks = max_chunks.saturating_sub(1).max(2);
        max_chunk_chars = max_chunk_chars.saturating_sub(150).max(650);
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

    let source_preview = source_index
        .chunks
        .first()
        .map(|chunk| {
            if chunk.content.chars().count() > 350 {
                let truncated: String = chunk.content.chars().take(350).collect();
                format!("{truncated}...")
            } else {
                chunk.content.clone()
            }
        })
        .unwrap_or_default();

    let mut file_memory = parsed.memory.clone();
    if file_memory.symbols.len() > max_file_symbols {
        file_memory.symbols.truncate(max_file_symbols);
    }
    if file_memory.imports.len() > max_file_imports {
        file_memory.imports.truncate(max_file_imports);
    }
    file_memory.symbol_count = file_memory.symbols.len();
    file_memory.import_count = file_memory.imports.len();

    let source_chars: usize = source_preview.chars().count();

    debug!(
        target_file = %parsed.relative_path,
        profile = ?profile,
        chunk_count = parsed.source_index.chunks.len(),
        source_chars,
        symbol_count = file_memory.symbol_count,
        import_count = file_memory.import_count,
        "file_prompt_context_breakdown"
    );

    serde_json::to_string(&serde_json::json!({
        "path": parsed.relative_path,
        "language": parsed.language,
        "source_preview": source_preview,
        "file_memory_hint": {
            "symbol_count": file_memory.symbol_count,
            "import_count": file_memory.import_count,
            "top_symbols": file_memory.symbols.iter().take(8).map(|s| serde_json::json!({
                "name": s.name,
                "kind": s.kind,
                "line": s.line,
            })).collect::<Vec<_>>(),
        },
        "memory_file_path": memory_file_path.display().to_string(),
        "source_index_file_path": source_index_file_path.display().to_string(),
        "source_query": {
            "file_path": parsed.relative_path,
            "chunk_ids": [0, 1],
            "max_chars": if matches!(profile, PromptProfile::Standard) { 3500 } else { 1800 }
        },
        "memory_query": {
            "file_path": parsed.relative_path,
            "max_global_symbols": relevant_memory.global_symbols.len().clamp(8, 20),
            "max_open_items": relevant_memory.open_items.len().clamp(4, 10),
            "max_links": relevant_memory.links.len().clamp(4, 14)
        },
        "project_memory_stats": {
            "file_count": relevant_memory.file_count,
            "unique_symbol_count": relevant_memory.unique_symbol_count
        }
    }))
    .map_err(|e| PlainSightError::InvalidState(format!("serializing file prompt input: {e}")))
}

fn sync_memory_snapshot(
    memory_file_path: &Path,
    project_memory: &ProjectMemory,
    reason: &str,
) -> PlainResult<()> {
    let serialized = serde_json::to_string_pretty(project_memory)
        .map_err(|e| PlainSightError::InvalidState(format!("serializing project memory: {e}")))?;
    fs::write(memory_file_path, &serialized).map_err(|e| {
        PlainSightError::io(
            format!("writing project memory '{}'", memory_file_path.display()),
            e,
        )
    })?;

    debug!(
        reason,
        memory_file = %memory_file_path.display(),
        memory_bytes = serialized.len(),
        file_count = project_memory.file_count,
        unique_symbol_count = project_memory.unique_symbol_count,
        global_symbols = project_memory.global_symbols.len(),
        open_items = project_memory.open_items.len(),
        links = project_memory.links.len(),
        "memory_snapshot_synced"
    );

    Ok(())
}

fn debug_current_memory(memory_file_path: &Path, target_file: &str) {
    if let Ok(meta) = fs::metadata(memory_file_path) {
        debug!(
            target_file,
            memory_file = %memory_file_path.display(),
            memory_bytes = meta.len(),
            "memory_snapshot_current"
        );
    }
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

fn should_retry_compact_ollama_error(err: &PlainSightError) -> bool {
    let lower = err.to_string().to_ascii_lowercase();
    lower.contains("request timeout")
        || lower.contains("timed out")
        || lower.contains("stopping")
        || lower.contains("killed")
        || lower.contains("connection")
        || lower.contains("json payload instead of markdown")
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
