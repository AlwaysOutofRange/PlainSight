use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
};

use crate::error::PlainSightError;

#[derive(Debug)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub path: PathBuf,
}

pub struct FilterOptions {
    pub extensions: Vec<&'static str>,
    pub exclude_directories: Vec<&'static str>,
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
                    .contains(&component_str)
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
                    && self.filter_options.extensions.contains(
                        &path
                            .extension()
                            .unwrap_or_default()
                            .to_str()
                            .unwrap_or_default(),
                    )
                {
                    let file_info = FileInfo {
                        name: path
                            .file_name()
                            .map(|file_name| file_name.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        size: fs::metadata(path.clone())
                            .map_err(|e| {
                                PlainSightError::io(
                                    format!("reading metadata for '{}'", path.display()),
                                    e,
                                )
                            })?
                            .len(),
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
