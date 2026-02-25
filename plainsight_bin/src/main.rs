use plainsight;
use std::path::Path;

#[tokio::main]
async fn main() {
    let app = match plainsight::PlainSight::new("/home/god/Projects/PlainSight/docs") {
        Ok(app) => app,
        Err(why) => {
            tracing::error!(error = %why, "initialization failed");
            eprintln!("Initialization failed. See logs for details.");
            std::process::exit(1);
        }
    };

    if let Err(why) = app
        .run_project("plain_sight", Path::new("/home/god/Projects/PlainSight"))
        .await
    {
        tracing::error!(error = %why, "generation failed");
        eprintln!("Generation failed. See logs for details.");
        std::process::exit(1);
    }
}
