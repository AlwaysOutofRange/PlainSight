use std::error::Error;

use crate::framework::{LangaugeAdapter, ParseInput, ParseOutput};

pub struct Registry {
    adapters: Vec<Box<dyn LangaugeAdapter>>,
}

impl Registry {
    pub fn new(adapters: Vec<Box<dyn LangaugeAdapter>>) -> Self {
        Self { adapters }
    }

    pub fn parse(
        &self,
        path: &std::path::Path,
        input: ParseInput,
    ) -> Result<ParseOutput, Box<dyn Error>> {
        for adapter in &self.adapters {
            if adapter.can_parse_path(path) {
                return Ok(adapter.parse(input));
            }
        }

        Err("Failed to parse input file. No valid adapter was found.".into())
    }
}
