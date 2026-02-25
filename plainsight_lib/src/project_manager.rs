use std::{
    collections::{BTreeMap, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{error::PlainSightError, memory::FileMemory};

#[derive(Debug)]
pub struct ProjectManager {
    docs_root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ProjectContext {
    docs_root: PathBuf,
    project_name: String,
    project_root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetaCache {
    pub files: BTreeMap<String, FileMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileMeta {
    pub hash: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub memory: Option<FileMemory>,
}

impl ProjectManager {
    pub fn new(docs_root: impl Into<PathBuf>) -> Self {
        Self {
            docs_root: docs_root.into(),
        }
    }

    pub fn new_project(
        &self,
        project_name: impl Into<String>,
        project_root: impl Into<PathBuf>,
    ) -> ProjectContext {
        ProjectContext {
            docs_root: self.docs_root.clone(),
            project_name: project_name.into(),
            project_root: project_root.into(),
        }
    }
}

impl ProjectContext {
    pub fn project_docs_path(&self) -> PathBuf {
        self.docs_root.join(&self.project_name)
    }

    pub fn files_root_path(&self) -> PathBuf {
        self.project_docs_path().join("files")
    }

    pub fn summary_path(&self) -> PathBuf {
        self.project_docs_path().join("summary.md")
    }

    pub fn architecture_path(&self) -> PathBuf {
        self.project_docs_path().join("architecture.md")
    }

    pub fn meta_path(&self) -> PathBuf {
        self.project_root.join(".meta.json")
    }

    pub fn file_docs_dir(&self, file_path: impl AsRef<Path>) -> Result<PathBuf, PlainSightError> {
        let relative = self.relative_file_path(file_path)?;
        Ok(self.files_root_path().join(relative))
    }

    pub fn file_summary_path(
        &self,
        file_path: impl AsRef<Path>,
    ) -> Result<PathBuf, PlainSightError> {
        Ok(self.file_docs_dir(file_path)?.join("summary.md"))
    }

    pub fn file_docs_path(&self, file_path: impl AsRef<Path>) -> Result<PathBuf, PlainSightError> {
        Ok(self.file_docs_dir(file_path)?.join("docs.md"))
    }

    pub fn ensure_project_structure(&self) -> Result<(), PlainSightError> {
        fs::create_dir_all(self.files_root_path())
            .map_err(|e| PlainSightError::io("creating project docs structure", e))?;
        self.ensure_markdown_file(self.summary_path())?;
        self.ensure_markdown_file(self.architecture_path())?;
        Ok(())
    }

    pub fn ensure_file_structure(
        &self,
        file_path: impl AsRef<Path>,
    ) -> Result<(), PlainSightError> {
        let file_dir = self.file_docs_dir(file_path)?;
        fs::create_dir_all(&file_dir).map_err(|e| {
            PlainSightError::io(
                format!("creating file docs directory '{}'", file_dir.display()),
                e,
            )
        })?;
        self.ensure_markdown_file(file_dir.join("summary.md"))?;
        self.ensure_markdown_file(file_dir.join("docs.md"))?;
        Ok(())
    }

    pub fn load_meta(&self) -> Result<MetaCache, PlainSightError> {
        let path = self.meta_path();
        if !path.exists() {
            return Ok(MetaCache::default());
        }

        let content = fs::read_to_string(&path).map_err(|e| {
            PlainSightError::io(format!("reading meta cache '{}'", path.display()), e)
        })?;

        serde_json::from_str(&content).map_err(|e| {
            PlainSightError::InvalidState(format!(
                "failed to parse meta cache '{}': {e}",
                path.display()
            ))
        })
    }

    pub fn save_meta(&self, meta: &MetaCache) -> Result<(), PlainSightError> {
        let content = serde_json::to_string_pretty(meta)
            .map_err(|e| PlainSightError::InvalidState(format!("serializing meta cache: {e}")))?;
        let path = self.meta_path();
        fs::write(&path, content).map_err(|e| {
            PlainSightError::io(format!("writing meta cache '{}'", path.display()), e)
        })?;
        Ok(())
    }

    pub fn ensure_meta_exists(&self) -> Result<MetaCache, PlainSightError> {
        let meta = self.load_meta()?;
        if !self.meta_path().exists() {
            self.save_meta(&meta)?;
        }
        Ok(meta)
    }

    pub fn hash_file(&self, file_path: impl AsRef<Path>) -> Result<String, PlainSightError> {
        let path = file_path.as_ref();
        let content = fs::read(path)
            .map_err(|e| PlainSightError::io(format!("hashing file '{}'", path.display()), e))?;
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        Ok(format!("{:x}", hasher.finish()))
    }

    pub fn needs_generation(
        &self,
        file_path: impl AsRef<Path>,
        meta: &MetaCache,
    ) -> Result<bool, PlainSightError> {
        let relative = self.relative_file_path(file_path.as_ref())?;
        let key = relative.to_string_lossy().to_string();
        let hash = self.hash_file(file_path.as_ref())?;

        let cached_hash = meta.files.get(&key).map(|f| f.hash.as_str());
        let summary_exists = self.file_summary_path(file_path.as_ref())?.exists();
        let docs_exists = self.file_docs_path(file_path.as_ref())?.exists();

        Ok(cached_hash != Some(hash.as_str()) || !summary_exists || !docs_exists)
    }

    fn relative_file_path(&self, file_path: impl AsRef<Path>) -> Result<PathBuf, PlainSightError> {
        let file_path = file_path.as_ref();
        let absolute = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            self.project_root.join(file_path)
        };

        absolute
            .strip_prefix(&self.project_root)
            .map(|p| p.to_path_buf())
            .map_err(|_| PlainSightError::PathOutsideProject {
                path: absolute,
                project_root: self.project_root.clone(),
            })
    }

    fn ensure_markdown_file(&self, file_path: PathBuf) -> Result<(), PlainSightError> {
        if !file_path.exists() {
            fs::write(&file_path, "").map_err(|e| {
                PlainSightError::io(
                    format!("creating markdown file '{}'", file_path.display()),
                    e,
                )
            })?;
        }
        Ok(())
    }
}
