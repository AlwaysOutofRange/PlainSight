use core_ir::{
    Capabilities, Capability, Diagnostic, FileIr, Import, LanguageId, Package, Severity, Span,
    Symbol, SymbolId, SymbolKind,
};

use crate::framework::{LangaugeAdapter, ParseInput, ParseOutput};

pub struct JavaAdapter;

impl LangaugeAdapter for JavaAdapter {
    fn can_parse_path(&self, path: &std::path::Path) -> bool {
        path.extension().map_or(false, |ext| ext == "java")
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::from(vec![Capability::Symbols, Capability::Imports])
    }

    fn parse(&self, input: ParseInput) -> ParseOutput {
        let ir = parse_file(&input);

        ParseOutput { ir }
    }
}

fn parse_file(input: &ParseInput) -> FileIr {
    let source = &input.source;
    let path = &input.path.0;
    let mut symbols = Vec::new();
    let mut package = None;
    let mut imports = Vec::new();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    // Parse the source code and populate the symbols vector
    let mut split_iter = source.split_whitespace();

    while let Some(token) = split_iter.next() {
        if token.starts_with("class") {
            let mut peekable = split_iter.clone().peekable();
            let class_name = peekable.peek().expect("failed to peek").to_string();

            let mut span = None;
            if let Some(start) = source.find(&class_name) {
                let end = start + class_name.len();
                span = Some(Span {
                    start: start as u32,
                    end: end as u32,
                });
            }

            symbols.push(Symbol {
                id: SymbolId(format!("{}:class_{}", path, class_name)),
                name: class_name,
                kind: SymbolKind::Class,
                span: span,
            });
        } else if token.starts_with("interface") {
            let mut peekable = split_iter.clone().peekable();
            let interface_name = peekable.peek().expect("failed to peek").to_string();

            let mut span = None;
            if let Some(start) = source.find(&interface_name) {
                let end = start + interface_name.len();
                span = Some(Span {
                    start: start as u32,
                    end: end as u32,
                });
            }

            symbols.push(Symbol {
                id: SymbolId(format!("{}:interface_{}", path, interface_name)),
                name: interface_name,
                kind: SymbolKind::Interface,
                span: span,
            });
        } else if token.starts_with("enum") {
            let mut peekable = split_iter.clone().peekable();
            let enum_name = peekable.peek().expect("failed to peek").to_string();

            let mut span = None;
            if let Some(start) = source.find(&enum_name) {
                let end = start + enum_name.len();
                span = Some(Span {
                    start: start as u32,
                    end: end as u32,
                });
            }

            symbols.push(Symbol {
                id: SymbolId(format!("{}:enum_{}", path, enum_name)),
                name: enum_name,
                kind: SymbolKind::Enum,
                span: span,
            });
        } else if token.starts_with("import") {
            let mut peekable = split_iter.clone().peekable();
            let mut is_static = false;
            if peekable.peek().expect("failed to peek").to_string() == "static" {
                is_static = true;
                peekable.next();
            }

            let mut import_name = peekable.peek().expect("failed to peek").to_string();
            import_name.remove(import_name.len() - 1);
            let mut span = None;
            if let Some(start) = source.find(&import_name) {
                let end = start + import_name.len();
                span = Some(Span {
                    start: start as u32,
                    end: end as u32,
                });
            }

            imports.push(Import {
                path: import_name,
                is_static,
                span,
            });
        } else if token.starts_with("package") {
            let mut peekable = split_iter.clone().peekable();
            let package_name = peekable.peek().expect("failed to peek").to_string();

            let mut span = None;
            if let Some(start) = source.find(&package_name) {
                let end = start + package_name.len();
                span = Some(Span {
                    start: start as u32,
                    end: end as u32,
                });
            }

            package = Some(Package {
                name: package_name,
                span,
            });
        }
    }

    if symbols.is_empty() {
        diagnostics.push(Diagnostic {
            severity: Severity::Info,
            message: "Java adapter: no top-level type symbols found (heuristic).".to_string(),
            span: None,
        });

        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            message: "Heuristic Java parser: results may be incomplete.".to_string(),
            span: None,
        })
    } else {
        diagnostics.push(Diagnostic {
            severity: Severity::Info,
            message: "Java adapter: extracted package/imports and top-level types (heuristic)."
                .to_string(),
            span: None,
        });

        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            message: "Heuristic Java parser: results may be incomplete.".to_string(),
            span: None,
        })
    }

    FileIr {
        language: LanguageId::Java,
        path: input.path.clone(),
        package,
        imports,
        symbols,
        diagnostics,
    }
}
