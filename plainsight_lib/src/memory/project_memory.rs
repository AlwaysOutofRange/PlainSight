use std::collections::{BTreeMap, BTreeSet};

use super::{CrossFileLink, FileMemory, GlobalSymbol, OpenItem, ProjectMemory};
use crate::memory::file_memory::is_valid_identifier;

const MAX_GLOBAL_SYMBOLS: usize = 300;
const MAX_OPEN_ITEMS: usize = 120;
const MAX_PROJECT_LINKS: usize = 400;

pub fn build_project_memory(files: &[FileMemory]) -> ProjectMemory {
    let mut by_symbol: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    let mut by_name: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();

    for file in files {
        for sym in &file.symbols {
            by_symbol
                .entry((sym.name.clone(), sym.kind.clone()))
                .or_default()
                .insert(file.path.clone());
            by_name
                .entry(sym.name.clone())
                .or_default()
                .entry(sym.kind.clone())
                .or_default()
                .insert(file.path.clone());
        }
    }

    let unique_symbol_count = by_symbol.len();
    let links = build_links(files, &by_symbol);
    let mut global_symbols = by_symbol
        .into_iter()
        .map(|((name, kind), paths)| GlobalSymbol {
            name,
            kind,
            defined_in: paths.into_iter().collect(),
        })
        .collect::<Vec<_>>();

    global_symbols.sort_by(|a, b| {
        b.defined_in
            .len()
            .cmp(&a.defined_in.len())
            .then_with(|| a.name.cmp(&b.name))
    });
    if global_symbols.len() > MAX_GLOBAL_SYMBOLS {
        global_symbols.truncate(MAX_GLOBAL_SYMBOLS);
    }

    let open_items = build_open_items(&by_name);

    ProjectMemory {
        file_count: files.len(),
        unique_symbol_count,
        files: files.to_vec(),
        global_symbols,
        open_items,
        links,
    }
}

fn build_open_items(
    by_name: &BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
) -> Vec<OpenItem> {
    let mut out = Vec::new();

    for (name, kinds) in by_name {
        if kinds.len() <= 1 {
            continue;
        }

        let mut files = BTreeSet::new();
        let mut kind_names = Vec::new();
        for (kind, paths) in kinds {
            kind_names.push(kind.clone());
            for path in paths {
                files.insert(path.clone());
            }
        }

        out.push(OpenItem {
            kind: "kind_conflict".to_string(),
            symbol: name.clone(),
            message: format!(
                "symbol '{}' appears with multiple kinds: {}",
                name,
                kind_names.join(", ")
            ),
            files: files.into_iter().take(12).collect(),
        });
    }

    out.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    if out.len() > MAX_OPEN_ITEMS {
        out.truncate(MAX_OPEN_ITEMS);
    }
    out
}

fn build_links(
    files: &[FileMemory],
    by_symbol: &BTreeMap<(String, String), BTreeSet<String>>,
) -> Vec<CrossFileLink> {
    let mut by_name: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for ((name, _kind), locations) in by_symbol {
        by_name
            .entry(name.clone())
            .or_default()
            .extend(locations.iter().cloned());
    }

    let mut links = Vec::new();
    let mut seen = BTreeSet::new();

    for file in files {
        for import in &file.imports {
            let candidates = import_symbol_candidates(import, &file.language);
            for candidate in candidates {
                let Some(destinations) = by_name.get(&candidate) else {
                    continue;
                };

                for to_file in destinations {
                    if to_file == &file.path {
                        continue;
                    }

                    let key = (
                        file.path.clone(),
                        to_file.clone(),
                        candidate.clone(),
                        "import".to_string(),
                    );
                    if !seen.insert(key) {
                        continue;
                    }

                    links.push(CrossFileLink {
                        from_file: file.path.clone(),
                        to_file: to_file.clone(),
                        symbol: candidate.clone(),
                        reason: "import".to_string(),
                    });
                }
            }
        }
    }

    links.sort_by(|a, b| {
        a.from_file
            .cmp(&b.from_file)
            .then_with(|| a.symbol.cmp(&b.symbol))
            .then_with(|| a.to_file.cmp(&b.to_file))
    });
    if links.len() > MAX_PROJECT_LINKS {
        links.truncate(MAX_PROJECT_LINKS);
    }
    links
}

pub(crate) fn import_symbol_candidates(import: &str, language: &str) -> Vec<String> {
    match language {
        "rust" => rust_import_candidates(import),
        "python" => python_import_candidates(import),
        "javascript" | "typescript" => js_ts_import_candidates(import),
        "java" | "kotlin" | "csharp" => dotted_import_candidates(import),
        "go" => go_import_candidates(import),
        _ => generic_import_candidates(import),
    }
}

fn push_candidate(out: &mut Vec<String>, token: &str) {
    if token.len() < 3 {
        return;
    }
    if !is_valid_identifier(token) {
        return;
    }

    let lowered = token.to_ascii_lowercase();
    if matches!(
        lowered.as_str(),
        "use"
            | "import"
            | "from"
            | "require"
            | "as"
            | "self"
            | "super"
            | "crate"
            | "mod"
            | "pub"
            | "const"
            | "static"
            | "class"
            | "interface"
            | "enum"
            | "type"
            | "struct"
            | "trait"
            | "include"
            | "include_next"
    ) {
        return;
    }

    out.push(token.to_string());
}

fn generic_import_candidates(import: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in import.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            current.push(ch);
        } else if !current.is_empty() {
            push_candidate(&mut out, &current);
            current.clear();
        }
    }
    if !current.is_empty() {
        push_candidate(&mut out, &current);
    }
    out
}

fn rust_import_candidates(import: &str) -> Vec<String> {
    let mut out = Vec::new();
    for token in import.split("::") {
        let cleaned = token.trim().trim_end_matches(';');
        if cleaned == "*" {
            continue;
        }
        if let Some(alias) = cleaned
            .strip_prefix('{')
            .and_then(|s| s.split_whitespace().next())
        {
            push_candidate(&mut out, alias.trim_matches(&['{', '}', ','][..]));
        }
        if let Some(alias_pos) = cleaned.find(" as ") {
            let alias = cleaned[alias_pos + 4..]
                .trim()
                .trim_matches(&['{', '}', ','][..]);
            push_candidate(&mut out, alias);
            continue;
        }
        let leaf = cleaned
            .trim_matches(&['{', '}', ',', ' '][..])
            .split(',')
            .next_back()
            .unwrap_or_default()
            .trim();
        push_candidate(&mut out, leaf);
    }
    out
}

fn python_import_candidates(import: &str) -> Vec<String> {
    let mut out = Vec::new();
    let line = import.trim();
    if line.starts_with("from ") && line.contains(" import ") {
        if let Some((_, rhs)) = line.split_once(" import ") {
            for piece in rhs.split(',') {
                let p = piece.trim();
                if let Some((left, alias)) = p.split_once(" as ") {
                    push_candidate(&mut out, alias.trim());
                    let leaf = left.split('.').next_back().unwrap_or_default();
                    push_candidate(&mut out, leaf.trim());
                } else {
                    let leaf = p.split('.').next_back().unwrap_or_default();
                    push_candidate(&mut out, leaf.trim());
                }
            }
        }
    } else if let Some(rest) = line.strip_prefix("import ") {
        for piece in rest.split(',') {
            let p = piece.trim();
            if let Some((left, alias)) = p.split_once(" as ") {
                push_candidate(&mut out, alias.trim());
                let leaf = left.split('.').next_back().unwrap_or_default();
                push_candidate(&mut out, leaf.trim());
            } else {
                let leaf = p.split('.').next_back().unwrap_or_default();
                push_candidate(&mut out, leaf.trim());
            }
        }
    }
    out
}

fn js_ts_import_candidates(import: &str) -> Vec<String> {
    let mut out = Vec::new();
    let line = import.trim();

    if line.starts_with("import ") {
        if let Some((lhs, _)) = line.split_once(" from ") {
            let left = lhs.trim_start_matches("import ").trim();
            if left.starts_with('{') && left.ends_with('}') {
                let inner = left.trim_start_matches('{').trim_end_matches('}');
                for piece in inner.split(',') {
                    let p = piece.trim();
                    if let Some((orig, alias)) = p.split_once(" as ") {
                        push_candidate(&mut out, alias.trim());
                        push_candidate(&mut out, orig.trim());
                    } else {
                        push_candidate(&mut out, p);
                    }
                }
            } else {
                for piece in left.split(',') {
                    push_candidate(&mut out, piece.trim());
                }
            }
        }
    } else if let Some((lhs, _)) = line.split_once("= require(") {
        let left = lhs
            .trim()
            .trim_start_matches("const ")
            .trim_start_matches("let ")
            .trim_start_matches("var ")
            .trim();
        push_candidate(&mut out, left);
    }

    out
}

fn dotted_import_candidates(import: &str) -> Vec<String> {
    let mut out = Vec::new();
    let line = import
        .trim()
        .trim_start_matches("import ")
        .trim_start_matches("using ")
        .trim_end_matches(';')
        .trim();
    let leaf = line.split('.').next_back().unwrap_or_default();
    push_candidate(&mut out, leaf.trim());
    out
}

fn go_import_candidates(import: &str) -> Vec<String> {
    let mut out = Vec::new();
    let line = import.trim();
    if !line.starts_with("import ") {
        return out;
    }
    let rest = line.trim_start_matches("import ").trim();
    if let Some(alias) = rest.split_whitespace().next()
        && !alias.starts_with('"')
        && alias != "."
        && alias != "_"
    {
        push_candidate(&mut out, alias);
    }
    out
}
