use plainsight;

#[tokio::main]
async fn main() {
    plainsight::init_logging();
    if let Err(why) = plainsight::run(&plainsight::PlainSightConfig {
        project_name: "plain_sight".to_string(),
        docs_root: "/home/outofrange/Projects/PlainSight/docs".into(),
        project_root: "/home/outofrange/Projects/PlainSight".into(),
    })
    .await
    {
        tracing::error!(error = %why, "generation failed");
        eprintln!("Generation failed. See logs for details.");
        std::process::exit(1);
    }
}
