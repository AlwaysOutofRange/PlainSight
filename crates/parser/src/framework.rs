use std::sync::Arc;

use core_ir::{Capabilities, FileIr, FilePath};

pub struct ParseInput {
    pub path: FilePath,
    pub source: Arc<str>,
}

pub struct ParseOutput {
    pub ir: FileIr,
}

pub trait LangaugeAdapter: Send + Sync {
    fn can_parse_path(&self, path: &std::path::Path) -> bool;
    fn capabilities(&self) -> Capabilities;
    fn parse(&self, input: ParseInput) -> ParseOutput;
}
