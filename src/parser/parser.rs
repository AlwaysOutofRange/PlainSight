use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use serde::Serialize;
use tree_sitter::{Language, Query, Tree};

use crate::parser::{
    ExtractKind, LanguageSpec,
    parser::utils::{cap_node, cap_text, cap_texts, collect_use_imports, extract_with_query},
    types,
};

#[derive(Default)]
struct QueryCache {
    queries: HashMap<ExtractKind, Result<Arc<Query>, String>>,
}

impl QueryCache {
    fn get_or_compile(
        &mut self,
        kind: ExtractKind,
        lang: Language,
        source: &str,
    ) -> Result<Arc<Query>, String> {
        self.queries
            .entry(kind)
            .or_insert_with(|| {
                Query::new(&lang.into(), source)
                    .map(Arc::new)
                    .map_err(|e| format!("Invalid {} query: {}", kind.as_str(), e))
            })
            .clone()
    }
}

#[derive(Debug, Default, Serialize)]
pub struct ParseResult {
    pub functions: Vec<types::Function>,
    pub types: Vec<types::Type>,
    pub imports: Vec<types::Import>,
    pub variables: Vec<types::Variable>,
}

pub struct Parser<S: LanguageSpec> {
    spec: S,
    parser: tree_sitter::Parser,
    query_cache: QueryCache,
}

impl<S: LanguageSpec> Parser<S> {
    pub fn new(spec: S) -> Self {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&spec.language())
            .expect("Failed to set Tree-sitter language");

        Self {
            spec,
            parser,
            query_cache: QueryCache::default(),
        }
    }

    pub fn parse_and_extract(&mut self, source: &str) -> Result<ParseResult, String> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| "Failed to parse source".to_string())?;

        // Check if there are duplicate functions in the vector
        let mut functions = self.extract_functions(&tree, source)?;
        let mut types = self.extract_types(&tree, source)?;
        let mut imports = self.extract_imports(&tree, source)?;
        let mut variables = self.extract_variables(&tree, source)?;

        // Make this cleaner
        let mut seen: HashSet<String> = HashSet::new();
        functions.retain(|f| seen.insert(f.name.clone()));
        seen.clear();
        types.retain(|t| seen.insert(t.name.clone()));
        seen.clear();
        imports.retain(|i| seen.insert(i.name.clone()));
        seen.clear();
        variables.retain(|v| seen.insert(v.name.clone()));
        seen.clear();

        Ok(ParseResult {
            functions,
            types,
            imports,
            variables,
        })
    }

    fn extract_functions(
        &mut self,
        tree: &Tree,
        source: &str,
    ) -> Result<Vec<types::Function>, String> {
        let query = self.compile_query(ExtractKind::Functions)?;
        let root = tree.root_node();

        extract_with_query(&query, root, source.as_bytes(), |q, m, src| {
            let name = cap_text(q, m, src, "name")?;
            let params = cap_text(q, m, src, "params").unwrap_or_default();
            let ret = cap_text(q, m, src, "ret").filter(|s| !s.is_empty() && s != "()");
            let vis = cap_text(q, m, src, "vis");
            let owner = cap_text(q, m, src, "impl_target");

            Some(types::Function {
                name,
                params_text: params,
                return_type: ret,
                visibility: vis,
                owner,
            })
        })
    }

    fn extract_types(&mut self, tree: &Tree, source: &str) -> Result<Vec<types::Type>, String> {
        let query = self.compile_query(ExtractKind::Types)?;
        let root = tree.root_node();

        struct TypeFragment {
            kind: Option<String>,
            vis: Option<String>,
            fields: Vec<String>,
        }

        let mut fragments: HashMap<String, TypeFragment> = HashMap::new();

        let _ = extract_with_query(&query, root, source.as_bytes(), |q, m, src| {
            let name = cap_text(q, m, src, "name")?;
            let kind = cap_text(q, m, src, "kind");
            let vis = cap_text(q, m, src, "vis");

            // Build field strings from parallel captures.
            let fields = build_field_strings(q, m, src);

            fragments
                .entry(name)
                .and_modify(|frag| {
                    frag.fields.extend(fields.clone());
                    if frag.kind.is_none() {
                        frag.kind = kind.clone();
                    }
                    if frag.vis.is_none() {
                        frag.vis = vis.clone();
                    }
                })
                .or_insert(TypeFragment { kind, vis, fields });

            None::<()>
        });

        let types = fragments
            .into_iter()
            .map(|(name, frag)| types::Type {
                name,
                kind: frag.kind,
                visibility: frag.vis,
                fields: frag.fields,
            })
            .collect();

        Ok(types)
    }

    fn extract_imports(&mut self, tree: &Tree, source: &str) -> Result<Vec<types::Import>, String> {
        let query = self.compile_query(ExtractKind::Imports)?;
        let root = tree.root_node();
        let src = source.as_bytes();

        // The query just matches `(use_declaration) @root`.
        // We collect imports by walking each declaration's subtree inside
        // the closure (avoids lifetime issues with collecting Nodes).
        let mut imports = Vec::new();

        let _ = extract_with_query(&query, root, src, |q, m, s| {
            let node = cap_node(q, m, "root")?;
            if let Some(arg) = node.child_by_field_name("argument") {
                collect_use_imports(arg, "", s, &mut imports);
            }
            None::<()>
        });

        Ok(imports)
    }

    fn extract_variables(
        &mut self,
        tree: &Tree,
        source: &str,
    ) -> Result<Vec<types::Variable>, String> {
        let query = self.compile_query(ExtractKind::Variables)?;
        let root = tree.root_node();

        extract_with_query(&query, root, source.as_bytes(), |q, m, src| {
            let name = cap_text(q, m, src, "name")?;
            let ty = cap_text(q, m, src, "type");
            let mut value = cap_text(q, m, src, "value");

            if let Some(ref v) = value {
                // Hardcode it for now
                if !v.contains("node.named_child") && !v.contains("names[") {
                    value = Some(v.chars().filter(|c| !c.is_whitespace()).collect())
                }
            }

            let vis = cap_text(q, m, src, "vis");
            let is_mut = cap_node(q, m, "mut").is_some();
            let is_const = cap_node(q, m, "const_keyword").is_some();
            let is_static = cap_node(q, m, "static_keyword").is_some();

            Some(types::Variable {
                name,
                type_text: ty,
                value,
                visibility: vis,
                is_mut,
                is_const,
                is_static,
            })
        })
    }

    fn compile_query(&mut self, kind: ExtractKind) -> Result<Arc<Query>, String> {
        let query_source = self.spec.query(kind);
        self.query_cache
            .get_or_compile(kind, self.spec.language(), &query_source)
    }
}

fn build_field_strings(query: &Query, m: &tree_sitter::QueryMatch, src: &[u8]) -> Vec<String> {
    let names = cap_texts(query, m, src, "field_name");
    let types = cap_texts(query, m, src, "field_type");
    let vis = cap_texts(query, m, src, "field_vis");

    if names.is_empty() && types.is_empty() {
        return Vec::new();
    }

    let len = names.len().max(types.len());
    let mut fields = Vec::with_capacity(len);

    for i in 0..len {
        let n = names.get(i).map(String::as_str).unwrap_or("_");
        let t = types.get(i).map(String::as_str);
        let v = vis.get(i).map(|s| s.trim()).filter(|s| !s.is_empty());

        let field = match (v, t) {
            (Some(vis), Some(ty)) => format!("{} {}: {}", vis, n, ty),
            (None, Some(ty)) => format!("{}: {}", n, ty),
            (Some(vis), None) => format!("{} {}", vis, n),
            (None, None) => n.to_string(),
        };
        fields.push(field);
    }

    fields
}

pub(self) mod utils {
    use std::str;
    use tree_sitter::{Node, Query, QueryCursor, QueryMatch, StreamingIterator};

    use crate::parser::types::Import;

    pub fn extract_with_query<T, F>(
        query: &Query,
        root: Node,
        source: &[u8],
        mut build: F,
    ) -> Result<Vec<T>, String>
    where
        F: FnMut(&Query, &QueryMatch, &[u8]) -> Option<T>,
    {
        let mut cursor = QueryCursor::new();
        let mut out = Vec::new();

        let mut matches = cursor.matches(query, root, source);
        while let Some(m) = matches.next() {
            if let Some(item) = build(query, m, source) {
                out.push(item);
            }
        }

        Ok(out)
    }

    pub fn cap_node<'a>(query: &Query, m: &'a QueryMatch, name: &str) -> Option<Node<'a>> {
        let names = query.capture_names();
        m.captures.iter().find_map(|cap| {
            let cap_name = names[cap.index as usize];
            (cap_name == name).then_some(cap.node)
        })
    }

    pub fn cap_text(query: &Query, m: &QueryMatch, source: &[u8], name: &str) -> Option<String> {
        cap_node(query, m, name).and_then(|node| {
            node.utf8_text(source)
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
    }

    pub fn cap_texts(query: &Query, m: &QueryMatch, src: &[u8], name: &str) -> Vec<String> {
        let names = query.capture_names();
        m.captures
            .iter()
            .filter_map(|cap| {
                let cname = names[cap.index as usize];
                if cname != name {
                    return None;
                }
                cap.node
                    .utf8_text(src)
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            })
            .collect()
    }

    // Only for rust
    pub fn collect_use_imports(node: Node, prefix: &str, src: &[u8], out: &mut Vec<Import>) {
        let text = |n: Node| -> String { n.utf8_text(src).unwrap_or("").trim().to_string() };

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
                    .map(|p| text(p))
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
}
