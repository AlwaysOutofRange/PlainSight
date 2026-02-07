mod rust;

pub use rust::RustSpec;
use tree_sitter::Language;

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum ExtractKind {
    Functions,
    Imports,
    Types,
    Variables,
}

impl ExtractKind {
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

    fn query(&self, kind: ExtractKind) -> String {
        let query_path = format!("querys/{}/{}.scm", self.id(), kind.as_str());
        let query = std::fs::read_to_string(query_path.clone()).unwrap_or_default();

        query
    }
}
