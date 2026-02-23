mod rust;

use std::path::{Path, PathBuf};

pub use rust::RustSpec;
use tree_sitter::{Language, Node};

use crate::error::PlainSightError;
use crate::parser::types::Import;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum ExtractKind {
    Functions,
    Imports,
    Types,
    Variables,
}

impl ExtractKind {
    pub const ALL: [ExtractKind; 4] = [
        ExtractKind::Functions,
        ExtractKind::Imports,
        ExtractKind::Types,
        ExtractKind::Variables,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            ExtractKind::Functions => "functions",
            ExtractKind::Imports => "imports",
            ExtractKind::Types => "types",
            ExtractKind::Variables => "variables",
        }
    }
}

pub trait LanguageSpec {
    fn id(&self) -> &'static str;
    fn language(&self) -> Language;

    fn query_root(&self) -> &'static Path {
        Path::new("querys")
    }

    fn supported_extract_kinds(&self) -> &'static [ExtractKind] {
        &ExtractKind::ALL
    }

    fn supports_kind(&self, kind: ExtractKind) -> bool {
        self.supported_extract_kinds().contains(&kind)
    }

    fn query_path(&self, kind: ExtractKind) -> PathBuf {
        self.query_root()
            .join(self.id())
            .join(format!("{}.scm", kind.as_str()))
    }

    fn query(&self, kind: ExtractKind) -> Result<String, PlainSightError> {
        let query_path = self.query_path(kind);
        std::fs::read_to_string(&query_path).map_err(|source| PlainSightError::QueryLoad {
            path: query_path,
            source,
        })
    }

    fn collect_imports(&self, _node: Node<'_>, _src: &[u8], _out: &mut Vec<Import>) {}

    fn normalize_variable_value(&self, value: Option<String>) -> Option<String> {
        value
    }
}
