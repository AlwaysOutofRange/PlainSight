use std::{collections::HashMap, sync::Arc};

use tree_sitter::{Language, Query, Tree};

use crate::parser::{
    ExtractKind, LanguageSpec,
    parser::utils::{cap_node, cap_text, cap_texts, extract_with_query},
    types::{self, Import, Variable},
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

#[derive(Debug, Default)]
pub struct ParseResult {
    pub functions: Vec<types::Function>,
    pub types: Vec<types::Type>,
    pub imports: Vec<Import>,
    pub variables: Vec<Variable>,
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

    pub fn parse_and_extract(&mut self, source: &str) -> ParseResult {
        let tree = self
            .parser
            .parse(source, None)
            .ok_or_else(|| "Failed to parse source".to_string())
            .unwrap();

        ParseResult {
            functions: self.extract_functions(&tree, source).unwrap(),
            types: self.extract_types(&tree, source).unwrap(),
            imports: self.extract_imports(&tree, source).unwrap(),
            variables: self.extract_variables(&tree, source).unwrap(),
        }
    }

    fn extract_functions(
        &mut self,
        tree: &Tree,
        source: &str,
    ) -> Result<Vec<types::Function>, String> {
        let root = tree.root_node();
        let query = self.compile_query(ExtractKind::Functions)?;

        extract_with_query(&query, root, source.as_bytes(), |query, m, src| {
            let name = cap_text(query, m, src, "name")?;
            let params = cap_text(query, m, src, "params")?;
            let ret = cap_text(query, m, src, "ret").filter(|s| !s.is_empty() && s != "()");

            Some(types::Function {
                name,
                params_text: params,
                return_type: ret,
            })
        })
    }

    pub fn extract_types(&mut self, tree: &Tree, source: &str) -> Result<Vec<types::Type>, String> {
        let root = tree.root_node();
        let query = self.compile_query(ExtractKind::Types)?;
        let mut frags: HashMap<String, Vec<String>> = HashMap::new();

        let _ = extract_with_query(&query, root, source.as_bytes(), |query, m, src| {
            let name = cap_text(query, m, src, "name")?;

            let field_names = cap_texts(query, m, src, "field_name");
            let field_types = cap_texts(query, m, src, "field_type");
            let field_vis = {
                let explicit = cap_texts(query, m, src, "field_vis");
                explicit
                    .into_iter()
                    .chain(std::iter::repeat(String::new()))
                    .take(field_names.len())
                    .collect::<Vec<_>>()
            };

            let fields = field_names
                .into_iter()
                .zip(field_types.into_iter())
                .zip(field_vis.into_iter())
                .map(|((name, ty), vis)| {
                    if vis.is_empty() {
                        format!("{}: {}", name, ty)
                    } else {
                        format!("{} {}: {}", vis.trim(), name, ty)
                    }
                })
                .collect::<Vec<String>>();

            if frags.contains_key(&name) {
                frags.get_mut(&name).unwrap().extend(fields.clone());
            } else {
                frags.insert(name.clone(), fields.clone());
            }

            None::<()>
        });

        let types = frags
            .into_iter()
            .map(|(name, fields)| Ok(types::Type { name, fields }))
            .collect();

        types
    }

    pub fn extract_imports(&mut self, tree: &Tree, source: &str) -> Result<Vec<Import>, String> {
        let root = tree.root_node();
        let query = self.compile_query(ExtractKind::Imports)?;

        extract_with_query(&query, root, source.as_bytes(), |query, m, src| {
            let name =
                cap_text(query, m, src, "alias").or_else(|| cap_text(query, m, src, "name"))?;
            let path = cap_text(query, m, src, "path").unwrap_or_default();
            let is_wildcard = cap_node(query, m, "wildcard").is_some();

            Some(Import {
                path,
                name,
                alias: cap_text(query, m, src, "alias"),
                is_wildcard,
            })
        })
    }

    pub fn extract_variables(
        &mut self,
        tree: &Tree,
        source: &str,
    ) -> Result<Vec<Variable>, String> {
        let root = tree.root_node();
        let query = self.compile_query(ExtractKind::Variables)?;

        extract_with_query(&query, root, source.as_bytes(), |query, m, src| {
            let name = cap_text(query, m, src, "name")?;
            let ty = cap_text(query, m, src, "type");
            let value = cap_text(query, m, src, "value");
            let is_mut = cap_node(query, m, "mut").is_some();
            let is_const = cap_node(query, m, "const_keyword").is_some();
            let is_static = cap_node(query, m, "static_keyword").is_some();

            Some(Variable {
                name,
                type_text: ty,
                value,
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

pub(self) mod utils {
    use std::str;
    use tree_sitter::{Node, Query, QueryCursor, QueryMatch, StreamingIterator};

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
            if let Some(item) = build(query, &m, source) {
                out.push(item);
            }
        }

        Ok(out)
    }

    pub fn cap_node<'a>(query: &Query, m: &'a QueryMatch, name: &str) -> Option<Node<'a>> {
        m.captures.iter().find_map(|cap| {
            let cap_name = query.capture_names()[cap.index as usize];
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
        m.captures
            .iter()
            .filter_map(|cap| {
                let cname = query.capture_names()[cap.index as usize];
                (cname == name).then(|| {
                    cap.node
                        .utf8_text(src)
                        .ok()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                })
            })
            .flatten()
            .collect()
    }
}
