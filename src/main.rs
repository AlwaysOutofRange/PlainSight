#![allow(dead_code)]

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use parser::Parser;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use crate::{
    error::PlainSightError,
    file_walker::{FileWalker, FilterOptions},
    ollama::{OllamaWrapper, Task},
    parser::RustSpec,
    project_manager::{MetaCache, ProjectManager},
};

mod error;
mod file_walker;
mod ollama;
mod parser;
mod project_manager;

const PROJECT_NAME: &str = "plain_sight";
const DOCS_ROOT: &str = "/home/outofrange/Projects/PlainSight/docs";
const PROJECT_ROOT: &str = "/home/outofrange/Projects/PlainSight";

#[derive(Debug, Clone)]
struct ParsedFile {
    path: PathBuf,
    relative_path: String,
    json: String,
}

#[tokio::main]
async fn main() {
    init_logging();

    if let Err(err) = run().await {
        error!(error = %err, "generation failed");
        eprintln!("Generation failed. See logs for details.");
        std::process::exit(1);
    }
}

fn init_logging() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_file(false)
        .with_line_number(false)
        .pretty()
        .init();
}

async fn run() -> Result<(), PlainSightError> {
    let manager = ProjectManager::new(DOCS_ROOT, PROJECT_NAME, PROJECT_ROOT);

    info!(project = PROJECT_NAME, "ensure_structure");
    manager.ensure_project_structure()?;
    let mut meta = manager.ensure_meta_exists()?;

    let files = discover_source_files()?;
    if files.is_empty() {
        warn!(
            project = PROJECT_NAME,
            "no source files found, skipping generation"
        );
        return Ok(());
    }

    let parsed_files = parse_project_files(&files, &manager)?;
    if parsed_files.is_empty() {
        return Err(PlainSightError::InvalidState(
            "no files could be parsed for documentation generation".to_string(),
        ));
    }

    let project_index_json = build_project_index_json(&parsed_files)?;
    let wrapper = OllamaWrapper::new();

    generate_summaries(&wrapper, &manager, &parsed_files).await?;
    unload_tasks(&wrapper, &[Task::Summarize, Task::ProjectSummary]).await;

    generate_docs(&wrapper, &manager, &parsed_files, &project_index_json).await?;
    unload_tasks(&wrapper, &[Task::Documentation, Task::Architecture]).await;

    update_meta_for_files(&manager, &mut meta, &parsed_files)?;

    info!(
        project = PROJECT_NAME,
        file_count = parsed_files.len(),
        project_summary_path = %manager.summary_path().display(),
        architecture_path = %manager.architecture_path().display(),
        "project documentation generation completed"
    );

    Ok(())
}

fn discover_source_files() -> Result<Vec<PathBuf>, PlainSightError> {
    let walker = FileWalker::with_filter(FilterOptions {
        extensions: vec!["rs"],
        exclude_directories: vec![".git", "target", "docs"],
    });

    let mut files: Vec<PathBuf> = walker
        .walk(PathBuf::from(PROJECT_ROOT))?
        .into_iter()
        .map(|f| f.path)
        .collect();

    files.sort();
    Ok(files)
}

fn parse_project_files(
    files: &[PathBuf],
    manager: &ProjectManager,
) -> Result<Vec<ParsedFile>, PlainSightError> {
    let mut parser = Parser::new(RustSpec::new(tree_sitter_rust::LANGUAGE.into()))?;
    let mut parsed_files = Vec::new();

    for path in files {
        let relative_path = relative_path_display(path);
        info!(target_file = %relative_path, "parse_source");

        if let Err(err) = manager.ensure_file_structure(path) {
            warn!(target_file = %relative_path, error = %err, "failed to ensure file docs structure; skipping file");
            continue;
        }

        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(err) => {
                warn!(target_file = %relative_path, error = %err, "failed reading source file; skipping file");
                continue;
            }
        };

        let parsed = match parser.parse_and_extract(&source) {
            Ok(parsed) => parsed,
            Err(err) => {
                warn!(target_file = %relative_path, error = %err, "failed parsing source file; skipping file");
                continue;
            }
        };

        let json = match serde_json::to_string_pretty(&parsed) {
            Ok(json) => json,
            Err(err) => {
                warn!(target_file = %relative_path, error = %err, "failed serializing parse result; skipping file");
                continue;
            }
        };

        parsed_files.push(ParsedFile {
            path: path.clone(),
            relative_path,
            json,
        });
    }

    Ok(parsed_files)
}

fn build_project_index_json(parsed_files: &[ParsedFile]) -> Result<String, PlainSightError> {
    let mut files = Vec::with_capacity(parsed_files.len());

    for parsed in parsed_files {
        let symbols: serde_json::Value = serde_json::from_str(&parsed.json).map_err(|e| {
            PlainSightError::InvalidState(format!(
                "deserializing parsed json for '{}' failed: {e}",
                parsed.relative_path
            ))
        })?;

        files.push(serde_json::json!({
            "path": parsed.relative_path,
            "symbols": symbols,
        }));
    }

    serde_json::to_string_pretty(&serde_json::json!({
        "project": PROJECT_NAME,
        "file_count": parsed_files.len(),
        "files": files,
    }))
    .map_err(|e| PlainSightError::InvalidState(format!("serializing project index: {e}")))
}

async fn generate_summaries(
    wrapper: &OllamaWrapper,
    manager: &ProjectManager,
    parsed_files: &[ParsedFile],
) -> Result<(), PlainSightError> {
    info!(file_count = parsed_files.len(), "summary_phase_start");
    let mut file_summaries: Vec<(String, String)> = Vec::with_capacity(parsed_files.len());

    for parsed in parsed_files {
        info!(
            target_file = %parsed.relative_path,
            model_name = Task::Summarize.model(),
            "generate_file_summary"
        );

        let start = Instant::now();
        let summary = wrapper
            .summarize(&parsed.json)
            .await
            .map_err(PlainSightError::Ollama)?;
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
            model_name = Task::Summarize.model(),
            elapsed = %elapsed,
            summary_len = summary.len(),
            summary_path = %summary_path.display(),
            "file summary generated"
        );
    }

    info!(
        model_name = Task::ProjectSummary.model(),
        summary_path = %manager.summary_path().display(),
        "generate_project_summary"
    );

    let start = Instant::now();
    let summary_context = build_project_summary_context(&file_summaries);
    let project_summary = wrapper
        .project_summary(PROJECT_NAME, &summary_context)
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
        model_name = Task::ProjectSummary.model(),
        elapsed = %elapsed,
        summary_len = project_summary.len(),
        summary_path = %project_summary_path.display(),
        "project summary generated"
    );

    Ok(())
}

async fn generate_docs(
    wrapper: &OllamaWrapper,
    manager: &ProjectManager,
    parsed_files: &[ParsedFile],
    project_index_json: &str,
) -> Result<(), PlainSightError> {
    info!(file_count = parsed_files.len(), "documentation_phase_start");

    for parsed in parsed_files {
        info!(
            target_file = %parsed.relative_path,
            model_name = Task::Documentation.model(),
            "generate_file_docs"
        );

        let start = Instant::now();
        let docs = wrapper
            .document(&parsed.json)
            .await
            .map_err(PlainSightError::Ollama)?;
        let elapsed = format_duration(start.elapsed());

        let docs_path = manager.file_docs_path(&parsed.path)?;
        fs::write(&docs_path, docs).map_err(|e| {
            PlainSightError::io(format!("writing docs output '{}'", docs_path.display()), e)
        })?;

        info!(
            target_file = %parsed.relative_path,
            model_name = Task::Documentation.model(),
            elapsed = %elapsed,
            docs_path = %docs_path.display(),
            "file docs generated"
        );
    }

    info!(
        model_name = Task::Architecture.model(),
        architecture_path = %manager.architecture_path().display(),
        "generate_architecture"
    );

    let start = Instant::now();
    let architecture = wrapper
        .architecture(PROJECT_NAME, project_index_json)
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
        model_name = Task::Architecture.model(),
        elapsed = %elapsed,
        architecture_len = architecture.len(),
        architecture_path = %architecture_path.display(),
        "architecture generated"
    );

    Ok(())
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

async fn unload_tasks(wrapper: &OllamaWrapper, tasks: &[Task]) {
    let mut seen_models: BTreeSet<&'static str> = BTreeSet::new();

    for task in tasks {
        let model_name = task.model();
        if !seen_models.insert(model_name) {
            continue;
        }

        info!(model_name, "unload_model");
        match wrapper.unload_model(model_name).await {
            Ok(()) => info!(model_name, "model unloaded"),
            Err(err) => warn!(model_name, error = %err, "failed unloading model; continuing"),
        }
    }
}

fn update_meta_for_files(
    manager: &ProjectManager,
    meta: &mut MetaCache,
    parsed_files: &[ParsedFile],
) -> Result<(), PlainSightError> {
    for parsed in parsed_files {
        manager.update_file_meta(&parsed.path, meta)?;
    }

    manager.save_meta(meta)
}

fn relative_path_display(path: &Path) -> String {
    path.strip_prefix(PROJECT_ROOT)
        .unwrap_or(path)
        .display()
        .to_string()
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
