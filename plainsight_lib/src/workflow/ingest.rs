use std::{
    fs,
    path::{Path, PathBuf},
};

use tracing::{debug, info, warn};

use crate::{
    config::SourceDiscoveryConfig,
    error::Result,
    file_walker::{FileWalker, FilterOptions},
    memory,
    project_manager::{FileMeta, MetaCache, ProjectContext},
    source_indexer,
};

use super::types::ParsedFile;

pub(crate) fn discover_source_files(
    project_root: &Path,
    config: &SourceDiscoveryConfig,
) -> Result<Vec<PathBuf>> {
    let walker = FileWalker::with_filter(FilterOptions {
        extensions: config.extensions.clone(),
        exclude_directories: config.exclude_directories.clone(),
    });

    let mut files: Vec<PathBuf> = walker
        .walk(project_root.to_path_buf())?
        .into_iter()
        .map(|f| f.path)
        .collect();

    files.sort();
    Ok(files)
}

pub(crate) fn parse_project_files(
    files: &[PathBuf],
    manager: &ProjectContext,
    project_root: &Path,
) -> Result<Vec<ParsedFile>> {
    let mut parsed_files = Vec::new();
    let mut skipped_file_count = 0usize;

    for path in files {
        let relative_path = relative_path_display(path, project_root);
        debug!(target_file = %relative_path, "index_source");

        if let Err(err) = manager.ensure_file_structure(path) {
            warn!(target_file = %relative_path, error = %err, "failed to ensure file docs structure; skipping file");
            skipped_file_count += 1;
            continue;
        }

        let hash = match manager.hash_file(path) {
            Ok(hash) => hash,
            Err(err) => {
                warn!(target_file = %relative_path, error = %err, "failed hashing source file; skipping file");
                skipped_file_count += 1;
                continue;
            }
        };

        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(err) => {
                warn!(target_file = %relative_path, error = %err, "failed reading source file; skipping file");
                skipped_file_count += 1;
                continue;
            }
        };

        let language = detect_language(path);
        let source_index = source_indexer::build_source_index(&source, language);
        let file_memory = memory::build_file_memory(&relative_path, language, &source);

        parsed_files.push(ParsedFile {
            path: path.clone(),
            relative_path,
            language: language.to_string(),
            hash,
            source_index,
            memory: file_memory,
        });
    }

    info!(
        total_files = files.len(),
        parsed_files = parsed_files.len(),
        skipped_files = skipped_file_count,
        "ingest_complete"
    );

    Ok(parsed_files)
}

pub(crate) fn update_meta_for_files(
    manager: &ProjectContext,
    meta: &mut MetaCache,
    parsed_files: &[ParsedFile],
) -> Result<()> {
    for parsed in parsed_files {
        meta.files.insert(
            parsed.relative_path.clone(),
            FileMeta {
                hash: parsed.hash.clone(),
            },
        );
    }

    manager.save_meta(meta)
}

fn detect_language(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "rs" => "rust",
        "py" => "python",
        "js" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        "go" => "go",
        "java" => "java",
        "kt" => "kotlin",
        "cs" => "csharp",
        "c" | "h" => "c",
        "cc" | "cpp" | "hpp" => "cpp",
        _ => "text",
    }
}

fn relative_path_display(path: &Path, project_root: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .display()
        .to_string()
}
