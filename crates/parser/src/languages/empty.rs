use crate::framework::{LangaugeAdapter, ParseInput, ParseOutput};
use core_ir::{Capabilities, Diagnostic, FileIr, LanguageId, Severity};

pub struct EmptyAdapter;

impl LangaugeAdapter for EmptyAdapter {
    fn can_parse_path(&self, _: &std::path::Path) -> bool {
        true
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::none()
    }

    fn parse(&self, input: ParseInput) -> ParseOutput {
        let ir = FileIr {
            language: LanguageId::Empty,
            path: input.path,
            package: None,
            imports: vec![],
            symbols: vec![],
            diagnostics: vec![Diagnostic {
                severity: Severity::Info,
                message: "This is an empty language adapter. This is only for testing!".to_string(),
                span: None,
            }],
        };

        ParseOutput { ir }
    }
}
