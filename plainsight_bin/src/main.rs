use clap::Parser;
use plainsight;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "plainsight")]
#[command(about = "Generate source documentation with local Ollama models")]
struct Cli {
    /// Project root directory to scan.
    #[arg(value_name = "PROJECT_ROOT", default_value = ".")]
    project_root: PathBuf,

    /// Docs output root directory.
    #[arg(long, value_name = "DOCS_ROOT", default_value = "docs")]
    docs_root: PathBuf,

    /// Project name used under docs root (defaults to project root folder name).
    #[arg(long, value_name = "NAME")]
    project_name: Option<String>,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let project_name = cli
        .project_name
        .unwrap_or_else(|| infer_project_name(&cli.project_root));

    let app = match plainsight::PlainSight::new(&cli.docs_root) {
        Ok(app) => app,
        Err(why) => {
            tracing::error!(error = %why, "initialization failed");
            eprintln!("Initialization failed. See logs for details.");
            std::process::exit(1);
        }
    };

    if let Err(why) = app.run_project(&project_name, &cli.project_root).await {
        tracing::error!(error = %why, "generation failed");
        eprintln!("Generation failed. See logs for details.");
        std::process::exit(1);
    }
}

fn infer_project_name(project_root: &std::path::Path) -> String {
    project_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(|name| name.replace('-', "_"))
        .unwrap_or_else(|| "plain_sight".to_string())
}
