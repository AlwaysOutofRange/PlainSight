use ollama_rs::{
    Ollama,
    generation::{completion::request::GenerationRequest, parameters::KeepAlive},
    models::ModelOptions,
};

#[derive(Debug, Clone, Copy)]
pub enum Task {
    Documentation,
    ProjectSummary,
    Architecture,
    Summarize,
}

impl Task {
    pub fn model(self) -> &'static str {
        match self {
            Task::Documentation => "qwen2.5-coder:7b",
            Task::ProjectSummary => "llama3.1:8b",
            Task::Architecture => "qwen2.5-coder:7b",
            Task::Summarize => "llama3.1:8b",
        }
    }

    pub fn temperature(self) -> f32 {
        match self {
            Task::Documentation => 0.1,
            Task::ProjectSummary => 0.2,
            Task::Architecture => 0.1,
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
        self.generate(task, prompt).await
    }

    pub async fn unload_task_model(&self, task: Task) -> Result<(), String> {
        self.unload_model(task.model()).await
    }

    pub async fn unload_model(&self, model_name: &str) -> Result<(), String> {
        // Ollama unload pattern: empty prompt + keep_alive=0.
        let request = GenerationRequest::new(model_name.to_string(), "")
            .keep_alive(KeepAlive::UnloadOnCompletion);

        self.client
            .generate(request)
            .await
            .map(|_| ())
            .map_err(|e| format!("failed to unload model ({}): {e}", model_name))
    }

    pub async fn summarize(&self, json_symbol_index: &str) -> Result<String, String> {
        let json = utils::prepare_file_summary_input(json_symbol_index)?;
        let task = Task::Summarize;
        let prompt = prompts::build_summary_prompt(&json);
        let out = self.generate(task, &prompt).await?;
        let out = utils::strip_wrapping_code_fence(out);
        utils::ensure_non_empty(task, out)
    }

    pub async fn document(&self, json_symbol_index: &str) -> Result<String, String> {
        let json = utils::prepare_file_docs_input(json_symbol_index)?;
        let task = Task::Documentation;
        let prompt = prompts::build_doc_prompt(&json);
        let out = self.generate(task, &prompt).await?;
        let out = utils::strip_wrapping_code_fence(out);
        utils::ensure_non_empty(task, out)
    }

    pub async fn project_summary(
        &self,
        project_name: &str,
        file_summaries_context: &str,
    ) -> Result<String, String> {
        let task = Task::ProjectSummary;
        let prompt = prompts::build_project_summary_prompt(project_name, file_summaries_context);
        let out = self.generate(task, &prompt).await?;
        let out = utils::strip_wrapping_code_fence(out);
        utils::ensure_non_empty(task, out)
    }

    pub async fn architecture(
        &self,
        project_name: &str,
        json_symbol_index: &str,
    ) -> Result<String, String> {
        let json = utils::prepare_architecture_input(json_symbol_index)?;
        let task = Task::Architecture;
        let prompt = prompts::build_architecture_prompt(project_name, &json);
        let out = self.generate(task, &prompt).await?;
        let out = utils::strip_wrapping_code_fence(out);
        utils::ensure_non_empty(task, out)
    }

    async fn generate(&self, task: Task, prompt: &str) -> Result<String, String> {
        let request = GenerationRequest::new(task.model().to_string(), prompt.to_string())
            .options(ModelOptions::default().temperature(task.temperature()));

        self.client
            .generate(request)
            .await
            .map(|r| r.response)
            .map_err(|e| format!("ollama error ({}): {e}", task.model()))
    }
}

mod prompts {
    pub fn build_summary_prompt(json: &str) -> String {
        format!(
            r#"
        You are summarizing one source file in a software codebase.

        Input is a parsed symbol index of that file.

        Write concise markdown with:
        - Responsibility: what this file owns
        - Key Types: core structs/enums/traits and roles
        - Key Flows: main functions and control/data flow

        Rules:
        - Max 180 words.
        - Do not dump raw JSON.
        - Do not invent symbols.
        - Be technical and specific.
        - Mention constraints/assumptions only if clearly implied.
        - Follow this format:
          ## Responsibility
          ...
          ## Key Types
          - ...
          ## Key Flows
          - ...

        Example output:
        ```md
        ## Responsibility
        Owns language-specific symbol extraction for one source file.

        ## Key Types
        - `Parser`: orchestrates query execution and result assembly.

        ## Key Flows
        - Parse source, run extract queries, deduplicate symbol lists.
        ```

        INPUT JSON:
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
        You are writing technical documentation for one source file.

        Input is a parsed symbol index for that file.

        Write markdown with exactly these sections:
        1) Overview
        2) Public API
        3) Internal Data Model
        4) Core Execution Flow
        5) Notes

        Rules:
        - Use only symbols present in the input.
        - Do not invent behavior.
        - Prefer bullets over long prose.
        - If details are missing, call out assumptions explicitly.
        - Keep it implementation-focused (not tutorial style).
        - Follow the exact section headers.

        Example output:
        ```md
        ## Overview
        ...
        ## Public API
        - `Foo::bar(...)`
        ## Internal Data Model
        ...
        ## Core Execution Flow
        - Step 1 ...
        ## Notes
        - ...
        ```

        INPUT JSON:
        ```json
        {}
        ```
        "#,
            json
        )
    }

    pub fn build_project_summary_prompt(
        project_name: &str,
        file_summaries_context: &str,
    ) -> String {
        format!(
            r#"
        You are writing the root-level project summary for `{}`.

        You are given summaries for project files.
        Produce `summary.md` as high-level project documentation.

        Write markdown with exactly these sections:
        1) Project Purpose
        2) Primary Capabilities
        3) Core Components
        4) How It Works (End-to-End)

        Rules:
        - This is project-level, not file-level.
        - Be honest that this is based on available snapshot context.
        - No invented modules or features.
        - Keep it concise and technical (around 200 words max).
        - Focus on what the project does and why.
        - Follow the exact section headers.

        Example output:
        ```md
        ## Project Purpose
        ...
        ## Primary Capabilities
        - ...
        ## Core Components
        - ...
        ## How It Works (End-to-End)
        - ...
        ```

        INPUT CONTEXT:
        ```md
        {}
        ```
        "#,
            project_name, file_summaries_context
        )
    }

    pub fn build_architecture_prompt(project_name: &str, json: &str) -> String {
        format!(
            r#"
        You are documenting architecture for `{}`.

        Use the parsed symbol index snapshot below and output markdown suitable for `architecture.md`.

        Write markdown with exactly these sections:
        1) Architectural Overview
        2) Key Modules and Responsibilities
        3) Data and Control Flow
        4) Integration Boundaries

        Rules:
        - Use only evidence from input.
        - Do not fabricate dependencies or runtime behavior.
        - Keep it concise and engineering-focused.
        - Follow the exact section headers.
        - Mention cross-module interactions explicitly.

        Example output:
        ```md
        ## Architectural Overview
        ...
        ## Key Modules and Responsibilities
        - `parser`: ...
        ## Data and Control Flow
        1. ...
        ## Integration Boundaries
        - ...
        ```

        INPUT JSON:
        ```json
        {}
        ```
        "#,
            project_name, json
        )
    }
}

mod utils {
    use serde_json::Value;
    use serde_json::json;

    use crate::ollama::Task;

    pub fn ensure_non_empty(task: Task, output: String) -> Result<String, String> {
        if output.trim().is_empty() {
            return Err(format!(
                "ollama returned empty output for task {:?} ({})",
                task,
                task.model()
            ));
        }
        Ok(output)
    }

    pub fn strip_wrapping_code_fence(output: String) -> String {
        let trimmed = output.trim();
        if !trimmed.starts_with("```") || !trimmed.ends_with("```") {
            return output;
        }

        let mut lines = trimmed.lines();
        let first = lines.next().unwrap_or_default();
        if !first.starts_with("```") {
            return output;
        }

        let mut body_lines: Vec<&str> = lines.collect();
        if body_lines.is_empty() {
            return output;
        }

        let last = body_lines.pop().unwrap_or_default();
        if last.trim() != "```" {
            return output;
        }

        body_lines.join("\n").trim().to_string()
    }

    pub fn prepare_file_summary_input(json: &str) -> Result<String, String> {
        let mut v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if let Some(obj) = v.as_object_mut() {
            obj.remove("imports");
            obj.remove("variables");
        }
        serde_json::to_string_pretty(&v).map_err(|e| e.to_string())
    }

    pub fn prepare_file_docs_input(json: &str) -> Result<String, String> {
        let mut v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if let Some(obj) = v.as_object_mut() {
            obj.remove("variables");
        }
        serde_json::to_string_pretty(&v).map_err(|e| e.to_string())
    }

    pub fn prepare_architecture_input(json: &str) -> Result<String, String> {
        build_project_digest(json, true)
    }

    fn build_project_digest(json: &str, include_import_names: bool) -> Result<String, String> {
        let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let files = v
            .get("files")
            .and_then(Value::as_array)
            .ok_or_else(|| "project index input missing 'files' array".to_string())?;

        let mut file_entries = Vec::with_capacity(files.len());
        for file in files {
            let path = file
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            let symbols = file.get("symbols").cloned().unwrap_or(Value::Null);

            let functions = collect_symbol_names(&symbols, "functions", 10);
            let types = collect_symbol_names(&symbols, "types", 10);
            let imports = collect_symbol_names(&symbols, "imports", 8);
            let variables = collect_symbol_names(&symbols, "variables", 8);

            let mut entry = json!({
                "path": path,
                "function_count": functions.len(),
                "type_count": types.len(),
                "import_count": imports.len(),
                "variable_count": variables.len(),
                "top_functions": functions,
                "top_types": types,
            });

            if include_import_names {
                entry["top_imports"] = json!(imports);
            }

            file_entries.push(entry);
        }

        let summary = json!({
            "project": v.get("project").cloned().unwrap_or(json!("unknown")),
            "file_count": v.get("file_count").cloned().unwrap_or(json!(file_entries.len())),
            "files": file_entries
        });

        serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())
    }

    fn collect_symbol_names(root: &Value, key: &str, max: usize) -> Vec<String> {
        root.get(key)
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.get("name").and_then(Value::as_str))
                    .take(max)
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }
}
