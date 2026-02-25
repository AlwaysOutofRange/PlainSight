mod client;
mod config;
mod prompts;
mod task;
mod utils;

pub use client::OllamaWrapper;
pub use config::{OllamaConfig, TaskConfig, TaskProfiles};
pub use task::Task;

pub fn is_refusal_output(output: &str) -> bool {
    utils::is_refusal_output(output)
}
