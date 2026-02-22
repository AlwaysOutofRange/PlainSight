#![allow(dead_code)]

use std::time::Instant;

use parser::Parser;
use tokio::io;

use crate::{ollama::OllamaWrapper, parser::RustSpec};

mod file_walker;
mod ollama;
mod parser;
mod project_manager;

#[tokio::main]
async fn main() {
    /*
    let walker = FileWalker::with_filter(FilterOptions {
        extensions: vec!["rs", "sh", "toml"],
        exclude_directories: vec!["target"],
    });

    let files = walker.walk(PathBuf::from(".")).unwrap();
    for file in files {
        println!("{:#?}", file);
    }
    */

    let mut parser = Parser::new(RustSpec::new(tree_sitter_rust::LANGUAGE.into()));
    let result = parser
        .parse_and_extract(include_str!("parser/parser.rs"))
        .unwrap();

    let json = serde_json::to_string_pretty(&result)
        .unwrap()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    let wrapper = OllamaWrapper::new();

    let start = Instant::now();

    let full = wrapper
        .summarize_stream_to(&json, io::stdout())
        .await
        .unwrap();

    let elapsed = start.elapsed();
    println!("\n\n---");
    println!("Total time: {:.2?}", elapsed);
    println!("Output length: {} chars", full.len());
}
