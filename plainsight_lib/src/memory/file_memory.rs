use std::collections::BTreeSet;

use super::{ConfidenceLevel, FileMemory, SymbolDetails, SymbolFact};

const MAX_FILE_SYMBOLS: usize = 200;
const MAX_FILE_IMPORTS: usize = 200;

pub fn build_file_memory(relative_path: &str, language: &str, source: &str) -> FileMemory {
    let mut symbols = Vec::new();
    let mut imports = Vec::new();

    for (idx, raw_line) in source.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comments(raw_line, language);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(import) = parse_import(trimmed, language) {
            imports.push(import);
        }

        if let Some(sym) = parse_symbol(trimmed, line_no, language) {
            symbols.push(sym);
        }
    }

    dedup_imports(&mut imports);
    dedup_symbols(&mut symbols);

    if symbols.len() > MAX_FILE_SYMBOLS {
        symbols.truncate(MAX_FILE_SYMBOLS);
    }
    if imports.len() > MAX_FILE_IMPORTS {
        imports.truncate(MAX_FILE_IMPORTS);
    }

    FileMemory {
        path: relative_path.to_string(),
        language: language.to_string(),
        symbol_count: symbols.len(),
        import_count: imports.len(),
        symbols,
        imports,
    }
}

fn strip_comments<'a>(line: &'a str, language: &str) -> &'a str {
    let marker = match language {
        "python" => "#",
        _ => "//",
    };
    line.split_once(marker)
        .map(|(left, _)| left)
        .unwrap_or(line)
}

fn parse_import(line: &str, language: &str) -> Option<String> {
    let is_import = match language {
        "rust" => line.starts_with("use "),
        "python" => line.starts_with("import ") || line.starts_with("from "),
        "javascript" | "typescript" => line.starts_with("import ") || line.contains("= require("),
        "go" => line.starts_with("import "),
        "java" | "kotlin" | "csharp" => line.starts_with("import ") || line.starts_with("using "),
        "c" | "cpp" => line.starts_with("#include "),
        _ => {
            line.starts_with("import ") || line.starts_with("use ") || line.starts_with("#include ")
        }
    };

    if !is_import {
        return None;
    }

    let mut normalized = line.trim_end_matches(';').to_string();
    if normalized.len() > 180 {
        normalized.truncate(180);
        normalized.push_str("...");
    }
    Some(normalized)
}

fn parse_symbol(line: &str, line_no: usize, language: &str) -> Option<SymbolFact> {
    let parsed = match language {
        "rust" => parse_rust_symbol(line),
        "python" => parse_python_symbol(line),
        "javascript" | "typescript" => parse_js_ts_symbol(line),
        "go" => parse_go_symbol(line),
        "java" | "kotlin" | "csharp" => parse_jvm_or_csharp_symbol(line),
        "c" | "cpp" => parse_c_family_symbol(line),
        _ => parse_fallback_symbol(line),
    }?;

    Some(SymbolFact {
        name: parsed.0,
        kind: parsed.1.to_string(),
        line: line_no,
        confidence: parsed.2,
        details: parsed.3,
    })
}

fn extract_identifier_after_keyword(line: &str, keyword: &str) -> Option<String> {
    let marker = format!("{keyword} ");
    let start = line.find(&marker)?;
    let rest = line.get(start + marker.len()..)?.trim_start();

    let mut out = String::new();
    for ch in rest.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            break;
        }
    }

    if out.is_empty() {
        return None;
    }

    if let Some(first) = out.chars().next()
        && first.is_ascii_digit()
    {
        return None;
    }

    Some(out)
}

fn parse_rust_symbol(line: &str) -> Option<(String, &'static str, ConfidenceLevel, SymbolDetails)> {
    let details = SymbolDetails::default();
    let candidates = [
        ("fn", "function"),
        ("struct", "struct"),
        ("enum", "enum"),
        ("trait", "trait"),
        ("mod", "module"),
        ("const", "const"),
        ("static", "static"),
        ("type", "type_alias"),
    ];

    for (keyword, kind) in candidates {
        if let Some(name) = extract_identifier_after_keyword(line, keyword) {
            return Some((name, kind, ConfidenceLevel::High, details));
        }
    }
    None
}

fn parse_python_symbol(
    line: &str,
) -> Option<(String, &'static str, ConfidenceLevel, SymbolDetails)> {
    let details = SymbolDetails::default();

    if let Some(name) = extract_identifier_after_keyword(line, "class") {
        return Some((name, "class", ConfidenceLevel::High, details));
    }

    if let Some(name) = extract_identifier_after_keyword(line, "def") {
        return Some((name, "function", ConfidenceLevel::High, details));
    }

    None
}

fn parse_js_ts_symbol(
    line: &str,
) -> Option<(String, &'static str, ConfidenceLevel, SymbolDetails)> {
    let details = SymbolDetails::default();
    let kind_candidates = [
        ("function", "function"),
        ("class", "class"),
        ("interface", "interface"),
        ("type", "type_alias"),
        ("enum", "enum"),
    ];

    for (keyword, kind) in kind_candidates {
        if let Some(name) = extract_identifier_after_keyword(line, keyword) {
            return Some((name, kind, ConfidenceLevel::High, details));
        }
    }

    if line.contains("=>") || (line.contains('(') && line.contains(')') && line.contains('{')) {
        if let Some(name) = extract_identifier_before_char(line, '(')
            && !is_control_keyword(&name)
        {
            return Some((name, "function", ConfidenceLevel::Medium, details));
        }
    }

    None
}

fn parse_go_symbol(line: &str) -> Option<(String, &'static str, ConfidenceLevel, SymbolDetails)> {
    let details = SymbolDetails::default();

    if line.starts_with("func ") {
        if line.starts_with("func (") {
            if let Some(name) = extract_identifier_after_char(line, ')') {
                return Some((name, "function", ConfidenceLevel::High, details));
            }
        } else if let Some(name) = extract_identifier_after_keyword(line, "func") {
            return Some((name, "function", ConfidenceLevel::High, details));
        }
    }

    for (keyword, kind) in [("type", "type"), ("const", "const"), ("var", "var")] {
        if let Some(name) = extract_identifier_after_keyword(line, keyword) {
            return Some((name, kind, ConfidenceLevel::High, details));
        }
    }

    None
}

fn parse_jvm_or_csharp_symbol(
    line: &str,
) -> Option<(String, &'static str, ConfidenceLevel, SymbolDetails)> {
    let details = SymbolDetails::default();

    for (keyword, kind) in [
        ("class", "class"),
        ("interface", "interface"),
        ("enum", "enum"),
        ("record", "record"),
    ] {
        if let Some(name) = extract_identifier_after_keyword(line, keyword) {
            return Some((name, kind, ConfidenceLevel::High, details));
        }
    }

    if line.contains('(') && line.contains(')') && line.ends_with('{') {
        if let Some(name) = extract_identifier_before_char(line, '(')
            && !is_control_keyword(&name)
        {
            return Some((name, "function", ConfidenceLevel::Medium, details));
        }
    }

    None
}

fn parse_c_family_symbol(
    line: &str,
) -> Option<(String, &'static str, ConfidenceLevel, SymbolDetails)> {
    let details = SymbolDetails::default();

    if let Some(name) = extract_identifier_after_keyword(line, "#define") {
        return Some((name, "macro", ConfidenceLevel::High, details));
    }

    for (keyword, kind) in [
        ("struct", "struct"),
        ("enum", "enum"),
        ("typedef", "type_alias"),
    ] {
        if let Some(name) = extract_identifier_after_keyword(line, keyword) {
            return Some((name, kind, ConfidenceLevel::High, details));
        }
    }

    if line.contains('(') && line.contains(')') && line.ends_with('{') {
        if let Some(name) = extract_identifier_before_char(line, '(')
            && !is_control_keyword(&name)
        {
            return Some((name, "function", ConfidenceLevel::Medium, details));
        }
    }

    None
}

fn parse_fallback_symbol(
    line: &str,
) -> Option<(String, &'static str, ConfidenceLevel, SymbolDetails)> {
    let details = SymbolDetails::default();

    for (keyword, kind) in [
        ("function", "function"),
        ("class", "class"),
        ("def", "function"),
    ] {
        if let Some(name) = extract_identifier_after_keyword(line, keyword) {
            return Some((name, kind, ConfidenceLevel::Low, details));
        }
    }

    None
}

fn extract_identifier_after_char(line: &str, ch: char) -> Option<String> {
    let idx = line.find(ch)?;
    let rest = line.get(idx + ch.len_utf8()..)?.trim_start();
    let mut out = String::new();
    for c in rest.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            break;
        }
    }
    if is_valid_identifier(&out) {
        Some(out)
    } else {
        None
    }
}

fn extract_identifier_before_char(line: &str, ch: char) -> Option<String> {
    let idx = line.find(ch)?;
    let prefix = line.get(..idx)?.trim_end();
    let token = prefix.split_whitespace().last()?.trim();
    if is_valid_identifier(token) {
        Some(token.to_string())
    } else {
        None
    }
}

pub(crate) fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let Some(first) = s.chars().next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_control_keyword(token: &str) -> bool {
    matches!(
        token,
        "if" | "for"
            | "while"
            | "switch"
            | "match"
            | "catch"
            | "foreach"
            | "loop"
            | "do"
            | "else"
            | "return"
    )
}

fn dedup_imports(imports: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    imports.retain(|item| seen.insert(item.clone()));
}

fn dedup_symbols(symbols: &mut Vec<SymbolFact>) {
    let mut seen = BTreeSet::new();
    symbols.retain(|item| {
        seen.insert((
            item.name.clone(),
            item.kind.clone(),
            item.line,
            item.confidence.clone(),
        ))
    });
}
