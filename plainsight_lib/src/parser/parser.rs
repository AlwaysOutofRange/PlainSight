use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use serde::Serialize;
use tree_sitter::{Language, Query, Tree};

use crate::error::PlainSightError;
use crate::parser::{
    ExtractKind, LanguageSpec,
    parser::utils::{cap_node, cap_text, cap_texts, extract_with_query},
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
                Query::new(&lang, source)
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
    pub fn new(spec: S) -> Result<Self, PlainSightError> {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&spec.language()).map_err(|e| {
            PlainSightError::Parse(format!("failed to set Tree-sitter language: {e}"))
        })?;

        Ok(Self {
            spec,
            parser,
            query_cache: QueryCache::default(),
        })
    }

    pub fn parse_and_extract(&mut self, source: &str) -> Result<ParseResult, PlainSightError> {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| PlainSightError::Parse("failed to parse source".to_string()))?;

        let mut functions = if self.spec.supports_kind(ExtractKind::Functions) {
            self.extract_functions(&tree, source)?
        } else {
            Vec::new()
        };

        let mut types = if self.spec.supports_kind(ExtractKind::Types) {
            self.extract_types(&tree, source)?
        } else {
            Vec::new()
        };

        let mut imports = if self.spec.supports_kind(ExtractKind::Imports) {
            self.extract_imports(&tree, source)?
        } else {
            Vec::new()
        };

        let mut variables = if self.spec.supports_kind(ExtractKind::Variables) {
            self.extract_variables(&tree, source)?
        } else {
            Vec::new()
        };

        dedup_by_key(&mut functions, |item| item.name.clone());
        dedup_by_key(&mut types, |item| item.name.clone());
        dedup_by_key(&mut imports, |item| item.name.clone());
        dedup_by_key(&mut variables, |item| item.name.clone());

        Ok(ParseResult {
            functions,
            types,
            imports,
            variables,
        })
    }

    pub fn validate_spec_queries(&mut self) -> Result<(), PlainSightError> {
        for kind in self.spec.supported_extract_kinds() {
            let _ = self.compile_query(*kind)?;
        }
        Ok(())
    }

    fn extract_functions(
        &mut self,
        tree: &Tree,
        source: &str,
    ) -> Result<Vec<types::Function>, PlainSightError> {
        let query = self.compile_query(ExtractKind::Functions)?;
        let root = tree.root_node();

        Ok(extract_with_query(
            &query,
            root,
            source.as_bytes(),
            |q, m, src| {
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
            },
        ))
    }

    fn extract_types(
        &mut self,
        tree: &Tree,
        source: &str,
    ) -> Result<Vec<types::Type>, PlainSightError> {
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

        Ok(fragments
            .into_iter()
            .map(|(name, frag)| types::Type {
                name,
                kind: frag.kind,
                visibility: frag.vis,
                fields: frag.fields,
            })
            .collect())
    }

    fn extract_imports(
        &mut self,
        tree: &Tree,
        source: &str,
    ) -> Result<Vec<types::Import>, PlainSightError> {
        let query = self.compile_query(ExtractKind::Imports)?;
        let root = tree.root_node();
        let src = source.as_bytes();

        let mut imports = Vec::new();

        let _ = extract_with_query(&query, root, src, |q, m, s| {
            let node = cap_node(q, m, "root")?;
            if let Some(arg) = node.child_by_field_name("argument") {
                self.spec.collect_imports(arg, s, &mut imports);
            }
            None::<()>
        });

        Ok(imports)
    }

    fn extract_variables(
        &mut self,
        tree: &Tree,
        source: &str,
    ) -> Result<Vec<types::Variable>, PlainSightError> {
        let query = self.compile_query(ExtractKind::Variables)?;
        let root = tree.root_node();

        Ok(extract_with_query(
            &query,
            root,
            source.as_bytes(),
            |q, m, src| {
                let name = cap_text(q, m, src, "name")?;
                let ty = cap_text(q, m, src, "type");
                let value = self
                    .spec
                    .normalize_variable_value(cap_text(q, m, src, "value"));

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
            },
        ))
    }

    fn compile_query(&mut self, kind: ExtractKind) -> Result<Arc<Query>, PlainSightError> {
        if !self.spec.supports_kind(kind) {
            return Err(PlainSightError::InvalidState(format!(
                "language '{}' does not support extract kind '{}'",
                self.spec.id(),
                kind.as_str()
            )));
        }

        let query_source = self.spec.query(kind)?;
        self.query_cache
            .get_or_compile(kind, self.spec.language(), &query_source)
            .map_err(|detail| PlainSightError::QueryCompile {
                kind: kind.as_str().to_string(),
                detail,
            })
    }
}

fn dedup_by_key<T, K, F>(items: &mut Vec<T>, mut key_fn: F)
where
    K: std::hash::Hash + Eq,
    F: FnMut(&T) -> K,
{
    let mut seen: HashSet<K> = HashSet::new();
    items.retain(|item| seen.insert(key_fn(item)));
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

mod utils {
    use tree_sitter::{Node, Query, QueryCursor, QueryMatch, StreamingIterator};

    pub fn extract_with_query<T, F>(
        query: &Query,
        root: Node,
        source: &[u8],
        mut build: F,
    ) -> Vec<T>
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

        out
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
}
