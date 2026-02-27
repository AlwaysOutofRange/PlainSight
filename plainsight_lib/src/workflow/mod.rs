mod generate;
mod ingest;
mod types;

use std::{collections::BTreeSet, fs, path::PathBuf};

use tracing::{info, warn};

use crate::{
    config::PlainSightConfig,
    error::{PlainSightError, Result},
    memory::{self, ProjectMemory},
    ollama::{OllamaWrapper, Task},
    project_manager::ProjectManager,
};

use types::ParsedFile;

pub(crate) async fn run_with_manager(
    manager: &ProjectManager,
    config: &PlainSightConfig,
    project_name: &str,
    project_root: &std::path::Path,
) -> Result<()> {
    let project = manager.new_project(project_name, project_root);

    info!(project = %project_name, "ensure_structure");
    project.ensure_project_structure()?;
    let mut meta = project.ensure_meta_exists()?;

    let files = ingest::discover_source_files(project_root, &config.source_discovery)?;
    if files.is_empty() {
        warn!(
            project = %project_name,
            "no source files found, skipping generation"
        );
        return Ok(());
    }

    let parsed_files = ingest::parse_project_files(&files, &project, project_root)?;
    if parsed_files.is_empty() {
        return Err(PlainSightError::InvalidState(
            "no files could be parsed for documentation generation".to_string(),
        ));
    }
    let files_to_regenerate: BTreeSet<String> = parsed_files
        .iter()
        .filter_map(
            |parsed| match project.needs_generation(&parsed.path, &meta) {
                Ok(true) => Some(Ok(parsed.relative_path.clone())),
                Ok(false) => None,
                Err(err) => Some(Err(err)),
            },
        )
        .collect::<Result<BTreeSet<_>>>()?;

    let project_memory = build_project_memory(&parsed_files);
    let memory_file_path = persist_project_memory(&project, &project_memory)?;
    let source_index_file_path = persist_source_index(&project, &parsed_files)?;
    let project_index = build_project_index(project_name, &parsed_files)?;
    let wrapper = OllamaWrapper::with_config(config.ollama.clone());

    generate::generate_summaries(
        &wrapper,
        &project,
        project_name,
        &parsed_files,
        &project_memory,
        &memory_file_path,
        &source_index_file_path,
        &files_to_regenerate,
    )
    .await?;
    generate::unload_tasks(&wrapper, &[Task::Summarize, Task::ProjectSummary]).await;

    generate::generate_docs(
        &wrapper,
        &project,
        project_name,
        &parsed_files,
        &project_memory,
        &memory_file_path,
        &source_index_file_path,
        &project_index,
        &files_to_regenerate,
    )
    .await?;
    generate::unload_tasks(&wrapper, &[Task::Documentation, Task::Architecture]).await;

    ingest::update_meta_for_files(&project, &mut meta, &parsed_files)?;

    info!(
        project = %project_name,
        file_count = parsed_files.len(),
        project_summary_path = %project.summary_path().display(),
        architecture_path = %project.architecture_path().display(),
        "project documentation generation completed"
    );

    Ok(())
}

fn persist_project_memory(
    project: &crate::project_manager::ProjectContext,
    project_memory: &ProjectMemory,
) -> Result<PathBuf> {
    let memory_file = project.project_docs_path().join(".memory.json");
    let memory_json = serde_json::to_string_pretty(project_memory)
        .map_err(|e| PlainSightError::InvalidState(format!("serializing project memory: {e}")))?;
    fs::write(&memory_file, memory_json).map_err(|e| {
        PlainSightError::io(
            format!("writing project memory '{}'", memory_file.display()),
            e,
        )
    })?;
    Ok(memory_file)
}

fn persist_source_index(
    project: &crate::project_manager::ProjectContext,
    parsed_files: &[ParsedFile],
) -> Result<PathBuf> {
    let source_index_file = project.project_docs_path().join(".source_index.json");

    let files = parsed_files
        .iter()
        .map(|parsed| {
            serde_json::json!({
                "path": parsed.relative_path,
                "language": parsed.language,
                "line_count": parsed.source_index.line_count,
                "chunk_count": parsed.source_index.chunk_count,
                "chunks": parsed.source_index.chunks,
            })
        })
        .collect::<Vec<_>>();

    let content = serde_json::to_string_pretty(&serde_json::json!({ "files": files }))
        .map_err(|e| PlainSightError::InvalidState(format!("serializing source index: {e}")))?;

    fs::write(&source_index_file, content).map_err(|e| {
        PlainSightError::io(
            format!("writing source index '{}'", source_index_file.display()),
            e,
        )
    })?;

    Ok(source_index_file)
}

fn build_project_memory(parsed_files: &[ParsedFile]) -> ProjectMemory {
    let files = parsed_files
        .iter()
        .map(|parsed| parsed.memory.clone())
        .collect::<Vec<_>>();
    memory::build_project_memory(&files)
}

fn build_project_index(project_name: &str, parsed_files: &[ParsedFile]) -> Result<String> {
    let mut files = Vec::with_capacity(parsed_files.len());

    for parsed in parsed_files {
        files.push(serde_json::json!({
            "path": parsed.relative_path,
            "symbols": &parsed.source_index,
        }));
    }

    serde_json::to_string_pretty(&serde_json::json!({
        "project": project_name,
        "file_count": parsed_files.len(),
        "files": files,
    }))
    .map_err(|e| PlainSightError::InvalidState(format!("serializing project index: {e}")))
}
