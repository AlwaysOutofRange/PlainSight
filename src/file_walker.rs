use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
};

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

#[derive(Default)]
pub struct FileWalker {
    filter_options: Option<FilterOptions>,
}

impl FileWalker {
    pub fn with_filter(filter_options: FilterOptions) -> Self {
        Self {
            filter_options: Some(filter_options),
        }
    }

    fn is_directory_excluded(&self, path: &Path, filter_options: &FilterOptions) -> bool {
        for component in path.components() {
            if let std::path::Component::Normal(os_str) = component {
                if let Some(component_str) = os_str.to_str() {
                    if filter_options.exclude_directories.contains(&component_str) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn walk(&self, path: PathBuf) -> Result<Vec<FileInfo>, Box<dyn std::error::Error>> {
        let mut directory_stack: VecDeque<PathBuf> = VecDeque::from([path]);
        let mut files: Vec<FileInfo> = Vec::new();

        let filter_options = self.filter_options.as_ref().unwrap();

        while let Some(current_path) = directory_stack.pop_front() {
            if self.is_directory_excluded(&current_path, filter_options) {
                continue;
            }

            for entry in fs::read_dir(&current_path).unwrap() {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    directory_stack.push_back(path);
                } else {
                    if !filter_options.extensions.is_empty()
                        && filter_options.extensions.contains(
                            &path
                                .extension()
                                .unwrap_or_default()
                                .to_str()
                                .unwrap_or_default(),
                        )
                    {
                        let file_info = FileInfo {
                            name: if let Some(file_name) = path.file_name() {
                                file_name.to_string_lossy().into_owned()
                            } else {
                                String::new()
                            },
                            size: fs::metadata(path.clone())?.len(),
                            path: path.canonicalize()?,
                        };
                        files.push(file_info);
                    }
                }
            }
        }

        Ok(files)
    }
}
