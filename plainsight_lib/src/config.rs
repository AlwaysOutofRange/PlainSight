use crate::ollama::OllamaConfig;

#[derive(Debug, Clone)]
pub struct SourceDiscoveryConfig {
    pub extensions: Vec<String>,
    pub exclude_directories: Vec<String>,
}

impl Default for SourceDiscoveryConfig {
    fn default() -> Self {
        Self {
            extensions: vec![
                "rs", "py", "js", "jsx", "ts", "tsx", "go", "java", "kt", "c", "h", "cc", "cpp",
                "hpp", "cs",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            exclude_directories: vec![".git", "target", "docs"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PlainSightConfig {
    pub source_discovery: SourceDiscoveryConfig,
    pub ollama: OllamaConfig,
}
