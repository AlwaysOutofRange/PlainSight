use ollama_rs::{Ollama, generation::completion::request::GenerationRequest, models::ModelOptions};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio_stream::StreamExt;

pub trait StreamSink {
    async fn on_chunk(&mut self, chunk: &str) -> Result<(), String>;
}

pub struct AsyncWriteSink<W: AsyncWrite + Unpin> {
    writer: W,
}

impl<W: AsyncWrite + Unpin> AsyncWriteSink<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W: AsyncWrite + Unpin> StreamSink for AsyncWriteSink<W> {
    async fn on_chunk(&mut self, chunk: &str) -> Result<(), String> {
        self.writer
            .write_all(chunk.as_bytes())
            .await
            .map_err(|e| format!("write error: {e}"))?;
        self.writer
            .flush()
            .await
            .map_err(|e| format!("flush error: {e}"))?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Task {
    Documentation,
    Summarize,
}

impl Task {
    pub fn model(self) -> &'static str {
        match self {
            Task::Documentation => "qwen2.5-coder:7b",
            Task::Summarize => "llama3.1:8b",
        }
    }

    pub fn temperature(self) -> f32 {
        match self {
            Task::Documentation => 0.1,
            Task::Summarize => 0.3,
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
        writer: W,
    ) -> Result<String, String> {
        let mut sink = AsyncWriteSink::new(writer);
        self.summarize_stream_with_sink(json_symbol_index, &mut sink)
            .await
    }

    pub async fn document_stream_to<W: AsyncWrite + Unpin>(
        &self,
        json_symbol_index: &str,
        writer: W,
    ) -> Result<String, String> {
        let mut sink = AsyncWriteSink::new(writer);
        self.document_stream_with_sink(json_symbol_index, &mut sink)
            .await
    }

    async fn summarize_stream_with_sink<S: StreamSink>(
        &self,
        json_symbol_index: &str,
        sink: &mut S,
    ) -> Result<String, String> {
        // Strip not needed keys from json
        let mut json = utils::strip_from_json(json_symbol_index, "imports")?;
        json = utils::strip_from_json(&json, "variables")?;

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
                sink.on_chunk(&resp.response).await?;
            }
        }

        Ok(full)
    }

    async fn document_stream_with_sink<S: StreamSink>(
        &self,
        json_symbol_index: &str,
        sink: &mut S,
    ) -> Result<String, String> {
        // Strip not needed keys from json
        let mut json = utils::strip_from_json(json_symbol_index, "imports")?;
        json = utils::strip_from_json(&json, "variables")?;

        let task = Task::Documentation;
        let prompt = prompts::build_doc_prompt(&json);

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
                sink.on_chunk(&resp.response).await?;
            }
        }

        Ok(full)
    }
}

pub(self) mod prompts {
    pub fn build_summary_prompt(json: &str) -> String {
        format!(
            r#"
        You are summarizing a Rust source file.

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
        ```
        "#,
            json
        )
    }

    pub fn build_doc_prompt(json: &str) -> String {
        format!(
            r#"
        You are writing technical documentation for a Rust source file.

        The following JSON represents its parsed symbol index.

        Write markdown documentation with these sections:
        - Overview
        - Public API
        - Internal Types and Data Flow
        - Key Functions
        - Edge Cases and Constraints

        Rules:
        - Use only symbols present in the input.
        - Do not invent behavior.
        - Keep it technical and concise.
        - If information is missing, state assumptions explicitly.

        INPUT:
        ```json
        {}
        ```
        "#,
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
