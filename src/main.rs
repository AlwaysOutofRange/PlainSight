#![allow(dead_code)]

use parser::Parser;

use crate::parser::RustSpec;

mod file_walker;
mod parser;

fn main() {
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
    let result = parser.parse_and_extract(include_str!("main.rs"));

    println!("Functions:");
    for f in &result.functions {
        println!("  {}({}) -> {:?}", f.name, f.params_text, f.return_type);
    }

    println!("\nTypes:");
    for t in &result.types {
        println!("  {} {{", t.name);
        for field in &t.fields {
            println!("    {}", field);
        }
        println!("  }}");
    }

    println!("\nImports:");
    for i in &result.imports {
        println!("  {:#?}", i);
    }

    println!("\nVariables:");
    for v in &result.variables {
        println!("  {:#?}", v);
    }
}
