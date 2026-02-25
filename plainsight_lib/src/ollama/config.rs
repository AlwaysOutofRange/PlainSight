use std::time::Duration;

use ollama_rs::models::ModelOptions;

use super::Task;

const DEFAULT_MODEL: &str = "phi4-mini:3.8b";

#[derive(Debug, Clone)]
pub struct TaskConfig {
    pub model: String,
    pub temperature: f32,
    pub num_ctx: u64,
    pub num_predict: i32,
    pub generate_timeout: Option<Duration>,
}

impl TaskConfig {
    pub fn options(&self) -> ModelOptions {
        ModelOptions::default()
            .temperature(self.temperature)
            .num_ctx(self.num_ctx)
            .num_predict(self.num_predict)
    }
}

#[derive(Debug, Clone)]
pub struct TaskProfiles {
    pub documentation: TaskConfig,
    pub project_summary: TaskConfig,
    pub architecture: TaskConfig,
    pub summarize: TaskConfig,
}

impl TaskProfiles {
    pub fn for_task(&self, task: Task) -> &TaskConfig {
        match task {
            Task::Documentation => &self.documentation,
            Task::ProjectSummary => &self.project_summary,
            Task::Architecture => &self.architecture,
            Task::Summarize => &self.summarize,
        }
    }

    pub fn set_model_for_all(&mut self, model: impl Into<String>) {
        let model = model.into();
        self.documentation.model = model.clone();
        self.project_summary.model = model.clone();
        self.architecture.model = model.clone();
        self.summarize.model = model;
    }
}

impl Default for TaskProfiles {
    fn default() -> Self {
        Self {
            documentation: TaskConfig {
                model: DEFAULT_MODEL.to_string(),
                temperature: 0.1,
                num_ctx: 4096,
                num_predict: 900,
                generate_timeout: None,
            },
            project_summary: TaskConfig {
                model: DEFAULT_MODEL.to_string(),
                temperature: 0.1,
                num_ctx: 4096,
                num_predict: 700,
                generate_timeout: None,
            },
            architecture: TaskConfig {
                model: DEFAULT_MODEL.to_string(),
                temperature: 0.1,
                num_ctx: 6144,
                num_predict: 1000,
                generate_timeout: None,
            },
            summarize: TaskConfig {
                model: DEFAULT_MODEL.to_string(),
                temperature: 0.2,
                num_ctx: 4096,
                num_predict: 300,
                generate_timeout: None,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct OllamaConfig {
    pub lock_timeout: Duration,
    pub unload_timeout: Duration,
    pub keep_alive_minutes: u64,
    pub tasks: TaskProfiles,
}

impl OllamaConfig {
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.tasks.set_model_for_all(model);
        self
    }
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            lock_timeout: Duration::from_secs(30),
            unload_timeout: Duration::from_secs(30),
            keep_alive_minutes: 30,
            tasks: TaskProfiles::default(),
        }
    }
}
