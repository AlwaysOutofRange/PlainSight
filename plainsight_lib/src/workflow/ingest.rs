use std::{
    fs,
    path::{Path, PathBuf},
};

use tracing::{info, warn};

use crate::{
    config::SourceDiscoveryConfig,
    error::PlainSightError,
    file_walker::{FileWalker, FilterOptions},
    memory::{self, FileMemory},
    project_manager::{FileMeta, MetaCache, ProjectContext},
    source_indexer,
};

use super::types::ParsedFile;

pub(crate) fn discover_source_files(
    project_root: &Path,
    config: &SourceDiscoveryConfig,
) -> Result<Vec<PathBuf>, PlainSightError> {
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
    meta: &MetaCache,
) -> Result<Vec<ParsedFile>, PlainSightError> {
    let mut parsed_files = Vec::new();

    for path in files {
        let relative_path = relative_path_display(path, project_root);
        info!(target_file = %relative_path, "index_source");

        if let Err(err) = manager.ensure_file_structure(path) {
            warn!(target_file = %relative_path, error = %err, "failed to ensure file docs structure; skipping file");
            continue;
        }

        let hash = match manager.hash_file(path) {
            Ok(hash) => hash,
            Err(err) => {
                warn!(target_file = %relative_path, error = %err, "failed hashing source file; skipping file");
                continue;
            }
        };

        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(err) => {
                warn!(target_file = %relative_path, error = %err, "failed reading source file; skipping file");
                continue;
            }
        };

        let language = detect_language(path);
        let source_index = source_indexer::build_source_index(&source, language);
        let file_memory = match cached_file_memory(meta, &relative_path, &hash, language) {
            Some(memory) => {
                info!(target_file = %relative_path, "reuse_file_memory");
                memory
            }
            None => memory::build_file_memory(&relative_path, language, &source),
        };

        parsed_files.push(ParsedFile {
            path: path.clone(),
            relative_path,
            language: language.to_string(),
            hash,
            source_index,
            memory: file_memory,
        });
    }

    Ok(parsed_files)
}

pub(crate) fn update_meta_for_files(
    manager: &ProjectContext,
    meta: &mut MetaCache,
    parsed_files: &[ParsedFile],
) -> Result<(), PlainSightError> {
    for parsed in parsed_files {
        meta.files.insert(
            parsed.relative_path.clone(),
            FileMeta {
                hash: parsed.hash.clone(),
                language: Some(parsed.language.clone()),
                memory: Some(parsed.memory.clone()),
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

fn cached_file_memory(
    meta: &MetaCache,
    relative_path: &str,
    hash: &str,
    language: &str,
) -> Option<FileMemory> {
    let cached = meta.files.get(relative_path)?;
    if cached.hash != hash {
        return None;
    }
    if cached.language.as_deref() != Some(language) {
        return None;
    }
    cached.memory.clone()
}
