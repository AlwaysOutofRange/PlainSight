use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceLevel {
    Low,
    #[default]
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolFact {
    pub name: String,
    pub kind: String,
    pub line: usize,
    #[serde(default)]
    pub confidence: ConfidenceLevel,
    #[serde(default)]
    pub details: SymbolDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SymbolDetails {
    #[serde(default)]
    pub visibility: String,
    #[serde(default)]
    pub modifiers: Vec<String>,
    #[serde(default)]
    pub signature: String,
    #[serde(default)]
    pub fields: Vec<FieldInfo>,
    #[serde(default)]
    pub variants: Vec<VariantInfo>,
    #[serde(default)]
    pub parameters: Vec<ParameterInfo>,
    #[serde(default)]
    pub return_type: String,
    #[serde(default)]
    pub generics: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldInfo {
    pub name: String,
    pub type_name: String,
    #[serde(default)]
    pub visibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantInfo {
    pub name: String,
    #[serde(default)]
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterInfo {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMemory {
    pub path: String,
    #[serde(default)]
    pub language: String,
    pub symbol_count: usize,
    pub import_count: usize,
    pub symbols: Vec<SymbolFact>,
    pub imports: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSymbol {
    pub name: String,
    pub kind: String,
    pub defined_in: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenItem {
    pub kind: String,
    pub symbol: String,
    pub message: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossFileLink {
    pub from_file: String,
    pub to_file: String,
    pub symbol: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMemory {
    pub file_count: usize,
    pub unique_symbol_count: usize,
    pub files: Vec<FileMemory>,
    pub global_symbols: Vec<GlobalSymbol>,
    #[serde(default)]
    pub open_items: Vec<OpenItem>,
    #[serde(default)]
    pub links: Vec<CrossFileLink>,
}
