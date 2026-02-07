use crate::parser::LanguageSpec;

pub struct RustSpec {
    lang: tree_sitter::Language,
}
impl RustSpec {
    pub fn new(lang: tree_sitter::Language) -> Self {
        Self { lang }
    }
}

impl LanguageSpec for RustSpec {
    fn id(&self) -> &'static str {
        "rust"
    }

    fn language(&self) -> tree_sitter::Language {
        self.lang.clone()
    }
}
