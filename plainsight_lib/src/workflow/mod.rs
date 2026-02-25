mod generate;
mod ingest;
mod types;

use tracing::{info, warn};

use crate::{
    config::PlainSightConfig,
    error::PlainSightError,
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
) -> Result<(), PlainSightError> {
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

    let parsed_files = ingest::parse_project_files(&files, &project, project_root, &meta)?;
    if parsed_files.is_empty() {
        return Err(PlainSightError::InvalidState(
            "no files could be parsed for documentation generation".to_string(),
        ));
    }

    let project_memory = build_project_memory(&parsed_files);
    let project_index = build_project_index(project_name, &parsed_files)?;
    let wrapper = OllamaWrapper::with_config(config.ollama.clone());

    generate::generate_summaries(
        &wrapper,
        &project,
        project_name,
        &parsed_files,
        &project_memory,
    )
    .await?;
    generate::unload_tasks(&wrapper, &[Task::Summarize, Task::ProjectSummary]).await;

    generate::generate_docs(
        &wrapper,
        &project,
        project_name,
        &parsed_files,
        &project_memory,
        &project_index,
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

fn build_project_memory(parsed_files: &[ParsedFile]) -> ProjectMemory {
    let files = parsed_files
        .iter()
        .map(|parsed| parsed.memory.clone())
        .collect::<Vec<_>>();
    memory::build_project_memory(&files)
}

fn build_project_index(
    project_name: &str,
    parsed_files: &[ParsedFile],
) -> Result<String, PlainSightError> {
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
