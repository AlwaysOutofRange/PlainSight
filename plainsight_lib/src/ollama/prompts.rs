use serde_json::{Map, Value, json};

const SUMMARY_INSTRUCTIONS: &str = concat!(
    "Write a concise summary of this source file. ",
    "Focus on the purpose, key functions/structs, and how it fits into the larger project. ",
    "Keep it under 150 words."
);

const DOCS_INSTRUCTIONS: &str = concat!(
    "Generate comprehensive documentation for this source file in the style of Rust's official docs (docs.rs). Include:\n",
    "1. Module-level documentation\n",
    "2. Public structs with fields and methods\n",
    "3. Public enums with variants\n",
    "4. Public functions with parameters and return values\n",
    "5. Examples where appropriate\n",
    "6. Error handling information\n",
    "Format with proper Markdown, code blocks, and parameter tables."
);

const PROJECT_SUMMARY_INSTRUCTIONS: &str = concat!(
    "Write a comprehensive project summary based on the individual file summaries. Include:\n",
    "1. Project overview and purpose\n",
    "2. Key components and their relationships\n",
    "3. Architecture patterns used\n",
    "4. Dependencies and external integrations\n",
    "5. Notable features or design decisions\n",
    "Keep it professional and suitable for documentation."
);

const ARCHITECTURE_INSTRUCTIONS: &str = concat!(
    "Generate comprehensive architecture documentation for this project. Include:\n",
    "1. System architecture overview\n",
    "2. Component diagrams and relationships\n",
    "3. Data flow and processing pipelines\n",
    "4. Key design patterns and decisions\n",
    "5. Performance considerations\n",
    "6. Scalability and maintenance aspects\n",
    "7. Security considerations if applicable\n",
    "Format with clear sections, diagrams (described in text), and technical details."
);

pub fn build_summary_prompt(context: &str) -> String {
    build_prompt(
        "summarize",
        SUMMARY_INSTRUCTIONS,
        [("context", json!(context))],
    )
}

pub fn build_doc_prompt(context: &str) -> String {
    build_prompt(
        "documentation",
        DOCS_INSTRUCTIONS,
        [("context", json!(context))],
    )
}

pub fn build_project_summary_prompt(project_name: &str, file_summaries: &str) -> String {
    build_prompt(
        "project_summary",
        PROJECT_SUMMARY_INSTRUCTIONS,
        [
            ("project_name", json!(project_name)),
            ("file_summaries", json!(file_summaries)),
        ],
    )
}

pub fn build_architecture_prompt(project_name: &str, context: &str) -> String {
    build_prompt(
        "architecture",
        ARCHITECTURE_INSTRUCTIONS,
        [
            ("project_name", json!(project_name)),
            ("context", json!(context)),
        ],
    )
}

fn build_prompt<const N: usize>(
    task: &str,
    instructions: &str,
    fields: [(&str, Value); N],
) -> String {
    let mut payload = Map::with_capacity(N + 2);
    for (key, value) in fields {
        payload.insert(key.to_string(), value);
    }
    payload.insert("task".to_string(), json!(task));
    payload.insert("instructions".to_string(), json!(instructions));

    serialize_prompt(&Value::Object(payload))
}

fn serialize_prompt(value: &Value) -> String {
    serde_json::to_string_pretty(value)
        .or_else(|_| serde_json::to_string(value))
        .unwrap_or_else(|_| "{\"task\":\"serialization_error\"}".to_string())
}
