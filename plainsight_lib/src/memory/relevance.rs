use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{CrossFileLink, GlobalSymbol, OpenItem, ProjectMemory};
use crate::memory::project_memory::import_symbol_candidates;

const MAX_RELEVANT_GLOBAL_SYMBOLS: usize = 40;
const MAX_RELEVANT_OPEN_ITEMS: usize = 10;
const MAX_RELEVANT_LINKS: usize = 20;
const RELEVANCE_SCORE_THRESHOLD: f32 = 0.3;

#[derive(Debug, Clone)]
pub struct SmartMemory {
    project_memory: ProjectMemory,
    import_export_graph: BTreeMap<String, BTreeSet<String>>,
}

impl SmartMemory {
    pub fn new(project_memory: ProjectMemory) -> Self {
        let mut import_export_graph = BTreeMap::new();

        for file in &project_memory.files {
            let mut imported_symbols = BTreeSet::new();
            for import in &file.imports {
                let candidates = import_symbol_candidates(import, &file.language);
                for candidate in candidates {
                    imported_symbols.insert(candidate);
                }
            }
            import_export_graph.insert(file.path.clone(), imported_symbols);
        }

        Self {
            project_memory,
            import_export_graph,
        }
    }

    pub fn get_relevant_memory_for_file(&self, file_path: &str) -> RelevantMemory {
        let relevance_scorer = RelevanceScorer::new(self, file_path);

        let mut scored_symbols: Vec<(usize, f32)> = self
            .project_memory
            .global_symbols
            .iter()
            .enumerate()
            .map(|(idx, symbol)| (idx, relevance_scorer.score_symbol(symbol)))
            .filter(|(_, score)| *score >= RELEVANCE_SCORE_THRESHOLD)
            .collect();

        scored_symbols.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let relevant_global_symbols: Vec<GlobalSymbol> = scored_symbols
            .iter()
            .take(MAX_RELEVANT_GLOBAL_SYMBOLS)
            .map(|(idx, _)| self.project_memory.global_symbols[*idx].clone())
            .collect();

        let mut scored_open_items: Vec<(usize, f32)> = self
            .project_memory
            .open_items
            .iter()
            .enumerate()
            .map(|(idx, item)| (idx, relevance_scorer.score_open_item(item)))
            .filter(|(_, score)| *score >= RELEVANCE_SCORE_THRESHOLD)
            .collect();

        scored_open_items
            .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let relevant_open_items: Vec<OpenItem> = scored_open_items
            .iter()
            .take(MAX_RELEVANT_OPEN_ITEMS)
            .map(|(idx, _)| self.project_memory.open_items[*idx].clone())
            .collect();

        let mut scored_links: Vec<(usize, f32)> = self
            .project_memory
            .links
            .iter()
            .enumerate()
            .map(|(idx, link)| (idx, relevance_scorer.score_link(link)))
            .filter(|(_, score)| *score >= RELEVANCE_SCORE_THRESHOLD)
            .collect();

        scored_links.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let relevant_links: Vec<CrossFileLink> = scored_links
            .iter()
            .take(MAX_RELEVANT_LINKS)
            .map(|(idx, _)| self.project_memory.links[*idx].clone())
            .collect();

        RelevantMemory {
            file_count: self.project_memory.file_count,
            unique_symbol_count: self.project_memory.unique_symbol_count,
            global_symbols: relevant_global_symbols,
            open_items: relevant_open_items,
            links: relevant_links,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelevantMemory {
    pub file_count: usize,
    pub unique_symbol_count: usize,
    pub global_symbols: Vec<GlobalSymbol>,
    pub open_items: Vec<OpenItem>,
    pub links: Vec<CrossFileLink>,
}

struct RelevanceScorer<'a> {
    smart_memory: &'a SmartMemory,
    target_file: &'a str,
    target_dir: PathBuf,
}

impl<'a> RelevanceScorer<'a> {
    fn new(smart_memory: &'a SmartMemory, target_file: &'a str) -> Self {
        let target_dir = Path::new(target_file)
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();

        Self {
            smart_memory,
            target_file,
            target_dir,
        }
    }

    fn score_symbol(&self, symbol: &GlobalSymbol) -> f32 {
        let mut score = 0.0;

        if symbol
            .defined_in
            .iter()
            .any(|path| path == self.target_file)
        {
            score += 1.0;
        }

        if let Some(imported_symbols) = self.smart_memory.import_export_graph.get(self.target_file)
            && imported_symbols.contains(&symbol.name)
        {
            score += 0.8;
        }

        for file_path in &symbol.defined_in {
            let symbol_dir = Path::new(file_path)
                .parent()
                .unwrap_or_else(|| Path::new(""));

            if symbol_dir == self.target_dir {
                score += 0.3;
            } else if self.is_subdirectory(symbol_dir, &self.target_dir) {
                score += 0.2;
            }
        }

        let usage_factor = 1.0 / (1.0 + (symbol.defined_in.len() as f32).log10());
        score * usage_factor
    }

    fn score_open_item(&self, item: &OpenItem) -> f32 {
        let mut score = 0.0;

        if item.files.iter().any(|path| path == self.target_file) {
            score += 1.0;
        }

        if let Some(imported_symbols) = self.smart_memory.import_export_graph.get(self.target_file)
            && imported_symbols.contains(&item.symbol)
        {
            score += 0.6;
        }

        for file_path in &item.files {
            let item_dir = Path::new(file_path)
                .parent()
                .unwrap_or_else(|| Path::new(""));

            if item_dir == self.target_dir {
                score += 0.4;
            } else if self.is_subdirectory(item_dir, &self.target_dir) {
                score += 0.2;
            }
        }

        score
    }

    fn score_link(&self, link: &CrossFileLink) -> f32 {
        let mut score = 0.0;

        if link.from_file == self.target_file || link.to_file == self.target_file {
            score += 1.0;
        }

        if let Some(imported_symbols) = self.smart_memory.import_export_graph.get(self.target_file)
            && imported_symbols.contains(&link.symbol)
        {
            score += 0.7;
        }

        let from_dir = Path::new(&link.from_file)
            .parent()
            .unwrap_or_else(|| Path::new(""));
        let to_dir = Path::new(&link.to_file)
            .parent()
            .unwrap_or_else(|| Path::new(""));

        if from_dir == self.target_dir || to_dir == self.target_dir {
            score += 0.3;
        } else if self.is_subdirectory(from_dir, &self.target_dir)
            || self.is_subdirectory(to_dir, &self.target_dir)
        {
            score += 0.15;
        }

        score
    }

    fn is_subdirectory(&self, potential_subdir: &Path, potential_parent: &Path) -> bool {
        potential_subdir.starts_with(potential_parent)
    }
}

pub fn get_relevant_memory_for_file(
    project_memory: &ProjectMemory,
    file_path: &str,
) -> RelevantMemory {
    let smart_memory = SmartMemory::new(project_memory.clone());
    smart_memory.get_relevant_memory_for_file(file_path)
}
