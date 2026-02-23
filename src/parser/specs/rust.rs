use tree_sitter::Node;

use crate::parser::{LanguageSpec, types::Import};

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

    fn collect_imports(&self, node: Node<'_>, src: &[u8], out: &mut Vec<Import>) {
        collect_use_imports(node, "", src, out);
    }

    fn normalize_variable_value(&self, value: Option<String>) -> Option<String> {
        value.map(|v| {
            if v.contains("node.named_child") || v.contains("names[") {
                v
            } else {
                v.chars().filter(|c| !c.is_whitespace()).collect()
            }
        })
    }
}

fn collect_use_imports(node: Node<'_>, prefix: &str, src: &[u8], out: &mut Vec<Import>) {
    let text = |n: Node<'_>| -> String { n.utf8_text(src).unwrap_or("").trim().to_string() };

    match node.kind() {
        "identifier" => {
            let name = text(node);
            if !name.is_empty() {
                out.push(Import {
                    path: prefix.to_string(),
                    name,
                    alias: None,
                    is_wildcard: false,
                });
            }
        }

        "self" => {
            out.push(Import {
                path: prefix.to_string(),
                name: "self".to_string(),
                alias: None,
                is_wildcard: false,
            });
        }

        "scoped_identifier" => {
            if let (Some(path_node), Some(name_node)) = (
                node.child_by_field_name("path"),
                node.child_by_field_name("name"),
            ) {
                let path_text = text(path_node);
                let name = text(name_node);
                let full_path = join_path(prefix, &path_text);
                if !name.is_empty() {
                    out.push(Import {
                        path: full_path,
                        name,
                        alias: None,
                        is_wildcard: false,
                    });
                }
            }
        }

        "scoped_use_list" => {
            if let Some(path_node) = node.child_by_field_name("path") {
                let path_text = text(path_node);
                let new_prefix = join_path(prefix, &path_text);

                if let Some(list_node) = node.child_by_field_name("list") {
                    collect_use_imports(list_node, &new_prefix, src, out);
                }
            }
        }

        "use_list" => {
            let count = node.named_child_count();
            for i in 0..count {
                if let Some(child) = node.named_child(i as u32) {
                    collect_use_imports(child, prefix, src, out);
                }
            }
        }

        "use_as_clause" => {
            if let (Some(path_node), Some(alias_node)) = (
                node.child_by_field_name("path"),
                node.child_by_field_name("alias"),
            ) {
                let path_text = text(path_node);
                let alias = text(alias_node);
                let full_path = join_path(prefix, &path_text);
                if !alias.is_empty() {
                    out.push(Import {
                        path: full_path,
                        name: alias.clone(),
                        alias: Some(alias),
                        is_wildcard: false,
                    });
                }
            }
        }

        "use_wildcard" => {
            let inner_path = node
                .child_by_field_name("path")
                .map(text)
                .unwrap_or_default();
            let full_path = join_path(prefix, &inner_path);

            out.push(Import {
                path: full_path,
                name: "*".to_string(),
                alias: None,
                is_wildcard: true,
            });
        }

        _ => {}
    }
}

fn join_path(prefix: &str, segment: &str) -> String {
    match (prefix.is_empty(), segment.is_empty()) {
        (true, _) => segment.to_string(),
        (_, true) => prefix.to_string(),
        _ => format!("{}::{}", prefix, segment),
    }
}
