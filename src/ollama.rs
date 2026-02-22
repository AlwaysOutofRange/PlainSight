use ollama_rs::{Ollama, generation::completion::request::GenerationRequest, models::ModelOptions};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio_stream::StreamExt;

#[derive(Debug, Clone, Copy)]
pub enum Task {
    Code,
    Reasoning,
    Chat,
    Summarize,
}

impl Task {
    pub fn model(self) -> &'static str {
        match self {
            Task::Code => "qwen2.5-coder:7b",
            Task::Reasoning => "deepseek-r1:1.5b",
            Task::Chat => "llama3.1:8b",
            Task::Summarize => "llama3.1:8b",
        }
    }

    pub fn temperature(self) -> f32 {
        match self {
            Task::Code => 0.1,
            Task::Reasoning => 0.2,
            Task::Summarize => 0.3,
            Task::Chat => 0.6,
        }
    }
}

pub struct OllamaWrapper {
    client: Ollama,
}

impl OllamaWrapper {
    pub fn new() -> Self {
        Self {
            client: Ollama::default(),
        }
    }

    pub async fn list_models(&self) -> Result<Vec<String>, String> {
        self.client
            .list_local_models()
            .await
            .map(|models| models.into_iter().map(|model| model.name).collect())
            .map_err(|e| format!("failed to list models: {e}"))
    }

    pub async fn generate_for_task(&self, task: Task, prompt: &str) -> Result<String, String> {
        let request = GenerationRequest::new(task.model().to_string(), prompt.to_string())
            .options(ModelOptions::default().temperature(task.temperature()));

        match self.client.generate(request).await {
            Ok(response) => Ok(response.response),
            Err(e) => Err(format!("ollama error ({}): {e}", task.model())),
        }
    }

    pub async fn summarize_stream_to<W: AsyncWrite + Unpin>(
        &self,
        json_symbol_index: &str,
        mut writer: W,
    ) -> Result<String, String> {
        // Strip not needed keys from json
        let mut json = utils::strip_from_json(&json_symbol_index, "imports").unwrap();
        json = utils::strip_from_json(&json, "variables").unwrap();

        let task = Task::Summarize;
        let prompt = prompts::build_summary_prompt(&json);

        let request = GenerationRequest::new(task.model().to_string(), prompt)
            .options(ModelOptions::default().temperature(task.temperature()));

        let mut stream = self
            .client
            .generate_stream(request)
            .await
            .map_err(|e| format!("ollama stream error ({}): {e}", task.model()))?;

        let mut full = String::new();

        while let Some(item) = stream.next().await {
            let responses =
                item.map_err(|e| format!("ollama stream chunk error ({}): {e}", task.model()))?;

            for resp in responses {
                full.push_str(&resp.response);

                writer
                    .write_all(resp.response.as_bytes())
                    .await
                    .map_err(|e| format!("write error: {e}"))?;
                writer
                    .flush()
                    .await
                    .map_err(|e| format!("flush error: {e}"))?;
            }
        }

        Ok(full)
    }
}

pub(self) mod prompts {
    pub fn build_summary_prompt(json: &str) -> String {
        format!(
            r#"You are summarizing a Rust source file.

The following JSON represents its parsed symbol index.

Write a concise summary (max 200 words) explaining:
- What the file is responsible for
- The main types
- The main operations

Rules:
- Do not restate JSON.
- Do not invent symbols.
- Be concise and technical.

INPUT:
```json
{}
```"#,
            json
        )
    }
}

pub(self) mod utils {
    pub fn strip_from_json(json: &str, key: &str) -> Result<String, String> {
        let mut v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if let Some(obj) = v.as_object_mut() {
            obj.remove(key);
        }
        serde_json::to_string_pretty(&v).map_err(|e| e.to_string())
    }
}
