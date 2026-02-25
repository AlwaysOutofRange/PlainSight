use std::path::Path;

use tracing_subscriber::EnvFilter;

use crate::{config::PlainSightConfig, error::PlainSightError, project_manager::ProjectManager};

pub mod config;
pub mod error;
pub mod file_walker;
pub mod memory;
pub mod ollama;
pub mod project_manager;
pub mod source_indexer;
mod workflow;

pub struct PlainSight {
    config: PlainSightConfig,
    manager: ProjectManager,
}

impl PlainSight {
    pub fn new(docs_root: impl AsRef<Path>) -> Result<Self, PlainSightError> {
        Self::with_config(docs_root, PlainSightConfig::default())
    }

    pub fn with_config(
        docs_root: impl AsRef<Path>,
        config: PlainSightConfig,
    ) -> Result<Self, PlainSightError> {
        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_target(true)
            .with_file(false)
            .with_line_number(false)
            .init();

        let docs_root = docs_root.as_ref().to_str().ok_or_else(|| {
            PlainSightError::InvalidState("docs_root contains non-utf8 characters".to_string())
        })?;

        Ok(Self {
            config,
            manager: ProjectManager::new(docs_root),
        })
    }

    pub async fn run_project(
        &self,
        project_name: &str,
        project_root: &Path,
    ) -> Result<(), PlainSightError> {
        workflow::run_with_manager(&self.manager, &self.config, project_name, project_root).await
    }

    pub fn manager(&self) -> &ProjectManager {
        &self.manager
    }

    pub fn config(&self) -> &PlainSightConfig {
        &self.config
    }
}
