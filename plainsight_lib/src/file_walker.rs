use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
};

use crate::error::PlainSightError;

#[derive(Debug)]
pub struct FileInfo {
    pub path: PathBuf,
}

pub struct FilterOptions {
    pub extensions: Vec<String>,
    pub exclude_directories: Vec<String>,
}

pub struct FileWalker {
    filter_options: FilterOptions,
}

impl FileWalker {
    pub fn with_filter(filter_options: FilterOptions) -> Self {
        Self { filter_options }
    }

    fn is_directory_excluded(&self, path: &Path) -> bool {
        for component in path.components() {
            if let std::path::Component::Normal(os_str) = component
                && let Some(component_str) = os_str.to_str()
                && self
                    .filter_options
                    .exclude_directories
                    .iter()
                    .any(|excluded| excluded == component_str)
            {
                return true;
            }
        }
        false
    }

    pub fn walk(&self, path: PathBuf) -> Result<Vec<FileInfo>, PlainSightError> {
        let mut directory_stack: VecDeque<PathBuf> = VecDeque::from([path]);
        let mut files: Vec<FileInfo> = Vec::new();

        while let Some(current_path) = directory_stack.pop_front() {
            if self.is_directory_excluded(&current_path) {
                continue;
            }

            let entries = fs::read_dir(&current_path).map_err(|e| {
                PlainSightError::io(format!("reading directory '{}'", current_path.display()), e)
            })?;

            for entry in entries {
                let entry = entry.map_err(|e| {
                    PlainSightError::io(
                        format!("reading entry in directory '{}'", current_path.display()),
                        e,
                    )
                })?;

                let path = entry.path();

                if path.is_dir() {
                    directory_stack.push_back(path);
                } else if !self.filter_options.extensions.is_empty()
                    && self.filter_options.extensions.iter().any(|ext| {
                        ext == path
                            .extension()
                            .unwrap_or_default()
                            .to_str()
                            .unwrap_or_default()
                    })
                {
                    let file_info = FileInfo {
                        path: path.canonicalize().map_err(|e| {
                            PlainSightError::io(format!("canonicalizing '{}'", path.display()), e)
                        })?,
                    };
                    files.push(file_info);
                }
            }
        }

        Ok(files)
    }
}
