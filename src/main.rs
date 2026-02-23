#![allow(dead_code)]

use std::{fs, path::PathBuf, time::Instant};

use parser::Parser;
use tokio::io;

use crate::{ollama::OllamaWrapper, parser::RustSpec, project_manager::ProjectManager};

mod file_walker;
mod ollama;
mod parser;
mod project_manager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = ProjectManager::new(
        "/home/god/Projects/PlainSight/docs",
        "plain_sight",
        "/home/god/Projects/PlainSight",
    );

    manager.ensure_project_structure()?;
    let mut meta = manager.ensure_meta_exists()?;

    let target_file = PathBuf::from("src/parser/parser.rs");

    if !manager.needs_generation(&target_file, &meta)? {
        println!(
            "No changes for '{}'. Skipping generation.",
            target_file.display()
        );
        return Ok(());
    }

    manager.ensure_file_structure(&target_file)?;

    let source = fs::read_to_string(&target_file)?;
    let mut parser = Parser::new(RustSpec::new(tree_sitter_rust::LANGUAGE.into()));
    let parsed = parser
        .parse_and_extract(&source)
        .map_err(std::io::Error::other)?;

    let json = serde_json::to_string_pretty(&parsed)?;
    let wrapper = OllamaWrapper::new();

    println!("Generating summary...");
    let start = Instant::now();
    let summary = wrapper
        .summarize_stream_to(&json, io::stdout())
        .await
        .map_err(std::io::Error::other)?;
    let summary_elapsed = start.elapsed();

    let summary_path = manager.file_summary_path(&target_file)?;
    fs::write(&summary_path, &summary)?;

    println!("\n\nGenerating docs...");
    let docs_start = Instant::now();
    let docs_path = manager.file_docs_path(&target_file)?;
    let docs_content = wrapper
        .document_stream_to(&json, io::stdout())
        .await
        .map_err(std::io::Error::other)?;
    let docs_elapsed = docs_start.elapsed();
    fs::write(&docs_path, docs_content)?;

    manager.update_file_meta(&target_file, &mut meta)?;
    manager.save_meta(&meta)?;

    println!("\n\n---");
    println!(
        "Summary time: {:.2?}, Docs time: {:.2?}",
        summary_elapsed, docs_elapsed
    );
    println!("Summary length: {} chars", summary.len());
    println!("Summary written to: {}", summary_path.display());
    println!("Docs written to: {}", docs_path.display());

    Ok(())
}
