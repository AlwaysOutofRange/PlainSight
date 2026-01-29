use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FilePath(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LanguageId {
    Java,
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolKind {
    Method,
    Class,
    Enum,
    Interface,
    Module,
    Variable,
    Constant,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Import {
    pub path: String,
    pub is_static: bool,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileIr {
    pub language: LanguageId,
    pub path: FilePath,
    pub package: Option<Package>,
    pub imports: Vec<Import>,
    pub symbols: Vec<Symbol>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    Symbols,
    Imports,
    BasicTypes,
    ControlFlow,
    DataFlow,
}

pub struct Capabilities {
    pub supported: Vec<Capability>,
}

impl Capabilities {
    pub fn none() -> Self {
        Capabilities {
            supported: Vec::new(),
        }
    }

    pub fn from(capabilities: Vec<Capability>) -> Self {
        Capabilities {
            supported: capabilities,
        }
    }

    pub fn supports(&self, capability: Capability) -> bool {
        self.supported.contains(&capability)
    }
}
