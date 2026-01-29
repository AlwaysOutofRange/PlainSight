use std::sync::Arc;

use argh::FromArgs;

#[derive(FromArgs)]
///
struct CliArgs {
    /// path to file
    #[argh(option)]
    path: String,
}

fn main() {
    let args: CliArgs = argh::from_env();

    let source = std::fs::read_to_string(&args.path)
        .expect("Failed to read file");

    let registry = parser::default_registry();
    let input = parser::framework::ParseInput {
        path: core_ir::FilePath(args.path.to_string()),
        source: Arc::from(source)
    };

    let out = registry.parse(std::path::Path::new(&args.path), input)
        .expect("failed to parse input");
    let json = serde_json::to_string_pretty(&out.ir)
        .expect("failed json");

    println!("{json}");
}
