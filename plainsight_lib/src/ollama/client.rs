use std::sync::Arc;

use ollama_rs::{
    Ollama,
    generation::{
        completion::request::GenerationRequest,
        parameters::{KeepAlive, TimeUnit},
    },
};
use tokio::sync::Semaphore;
use tokio::time;
use tracing::debug;

use super::{OllamaConfig, Task, prompts, utils};

pub struct OllamaWrapper {
    client: Ollama,
    config: OllamaConfig,
    lock: Arc<Semaphore>,
}

impl OllamaWrapper {
    pub fn new() -> Self {
        Self::with_config(OllamaConfig::default())
    }

    pub fn with_config(config: OllamaConfig) -> Self {
        Self {
            client: Ollama::default(),
            config,
            lock: Arc::new(Semaphore::new(1)),
        }
    }

    pub fn model_name(&self, task: Task) -> &str {
        &self.config.tasks.for_task(task).model
    }

    pub async fn list_models(&self) -> Result<Vec<String>, String> {
        self.client
            .list_local_models()
            .await
            .map(|models| models.into_iter().map(|model| model.name).collect())
            .map_err(|e| format!("failed to list models: {e}"))
    }

    pub async fn generate_for_task(&self, task: Task, prompt: &str) -> Result<String, String> {
        self.generate(task, prompt).await
    }

    pub async fn unload_task_model(&self, task: Task) -> Result<(), String> {
        self.unload_model(self.model_name(task)).await
    }

    pub async fn unload_model(&self, model_name: &str) -> Result<(), String> {
        let _permit = match time::timeout(self.config.lock_timeout, self.lock.acquire()).await {
            Ok(Ok(permit)) => permit,
            Ok(Err(e)) => return Err(format!("failed to acquire lock for unload: {e}")),
            Err(_) => {
                return Err(format!(
                    "timeout acquiring lock to unload model {}",
                    model_name
                ));
            }
        };

        let request = GenerationRequest::new(model_name.to_string(), "")
            .keep_alive(KeepAlive::UnloadOnCompletion);

        match time::timeout(self.config.unload_timeout, self.client.generate(request)).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(err)) => Err(format!("failed to unload model ({}): {err}", model_name)),
            Err(_) => {
                debug!(
                    model = model_name,
                    unload_timeout_secs = self.config.unload_timeout.as_secs(),
                    "unload timeout - connection may have been closed by Ollama or model is in 'Stopping...' state"
                );
                Ok(())
            }
        }
    }

    pub async fn summarize(&self, context_payload: &str) -> Result<String, String> {
        let context = utils::prepare_file_summary_input(context_payload)?;
        debug!(
            payload_bytes = context.len(),
            "ollama_summarize_payload_prepared"
        );
        let task = Task::Summarize;
        let prompt = prompts::build_summary_prompt(&context);
        debug!(
            prompt_bytes = prompt.len(),
            model = self.model_name(task),
            "ollama_summarize_prompt"
        );
        let out = self.generate(task, &prompt).await?;
        self.postprocess_output(task, out)
    }

    pub async fn document(&self, context_payload: &str) -> Result<String, String> {
        let context = utils::prepare_file_docs_input(context_payload)?;
        debug!(
            payload_bytes = context.len(),
            "ollama_docs_payload_prepared"
        );
        let task = Task::Documentation;
        let prompt = prompts::build_doc_prompt(&context);
        debug!(
            prompt_bytes = prompt.len(),
            model = self.model_name(task),
            "ollama_docs_prompt"
        );
        let out = self.generate(task, &prompt).await?;
        self.postprocess_output(task, out)
    }

    pub async fn project_summary(
        &self,
        project_name: &str,
        file_summaries_context: &str,
    ) -> Result<String, String> {
        let task = Task::ProjectSummary;
        let prompt = prompts::build_project_summary_prompt(project_name, file_summaries_context);
        debug!(
            prompt_bytes = prompt.len(),
            model = self.model_name(task),
            "ollama_project_summary_prompt"
        );
        let out = self.generate(task, &prompt).await?;
        self.postprocess_output(task, out)
    }

    pub async fn architecture(
        &self,
        project_name: &str,
        context_payload: &str,
    ) -> Result<String, String> {
        let context = utils::prepare_architecture_input(context_payload)?;
        debug!(
            payload_bytes = context.len(),
            "ollama_arch_payload_prepared"
        );
        let task = Task::Architecture;
        let prompt = prompts::build_architecture_prompt(project_name, &context);
        debug!(
            prompt_bytes = prompt.len(),
            model = self.model_name(task),
            "ollama_arch_prompt"
        );
        let out = self.generate(task, &prompt).await?;
        self.postprocess_output(task, out)
    }

    async fn generate(&self, task: Task, prompt: &str) -> Result<String, String> {
        let model_cfg = self.config.tasks.for_task(task);

        let _permit = match time::timeout(self.config.lock_timeout, self.lock.acquire()).await {
            Ok(Ok(permit)) => permit,
            Ok(Err(e)) => return Err(format!("failed to acquire lock: {e}")),
            Err(_) => {
                return Err(format!(
                    "timeout acquiring lock for model {}",
                    model_cfg.model
                ));
            }
        };

        let request = GenerationRequest::new(model_cfg.model.clone(), prompt.to_string())
            .keep_alive(KeepAlive::Until {
                time: self.config.keep_alive_minutes,
                unit: TimeUnit::Minutes,
            })
            .options(model_cfg.options());

        if let Some(generate_timeout) = model_cfg.generate_timeout {
            return match time::timeout(generate_timeout, self.client.generate(request)).await {
                Ok(Ok(response)) => Ok(response.response),
                Ok(Err(err)) => Err(format!("ollama error ({}): {err}", model_cfg.model)),
                Err(_) => Err(format!(
                    "ollama error ({}): request timeout after {} seconds - model may have been killed or is in 'Stopping...' state",
                    model_cfg.model,
                    generate_timeout.as_secs()
                )),
            };
        }

        self.client
            .generate(request)
            .await
            .map(|response| response.response)
            .map_err(|err| format!("ollama error ({}): {err}", model_cfg.model))
    }

    fn postprocess_output(&self, task: Task, out: String) -> Result<String, String> {
        let out = utils::strip_wrapping_code_fence(out);
        let out = utils::ensure_ai_disclaimer(out);
        utils::ensure_non_empty(task, self.model_name(task), out)
    }
}
