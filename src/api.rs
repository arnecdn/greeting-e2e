use std::path::{Path, PathBuf};
use clap::Parser;
use clio::Input;
use serde::{Deserialize, Serialize};
use validator_derive::Validate;
use log::{info, log};
use confy::ConfyError;

/// Runs e2e test for greeting-solution.
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub(crate) struct CliArgs {
    /// Path to configfile. If missing, a template file with default values is created.\n
    #[arg(short = 'c', long = "config")]
    pub config_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct E2ETestConfig {
    pub greeting_receiver_url: String,
    pub greeting_api_logg_url: String,
    pub num_iterations: i8
}

impl Default for E2ETestConfig {
    fn default() -> Self {
        E2ETestConfig {
            greeting_receiver_url: "http://localhost:80800".to_string(),
            greeting_api_logg_url: "http://localhost:80800".to_string(),
            num_iterations: 0,
        }
    }
}

pub (crate) fn load_e22_config(path: &str) -> Result<E2ETestConfig, ConfyError> {
    let config_path = Path::new(&path);
    let cfg: E2ETestConfig = confy::load_path(config_path)?;
    info!("Loaded E2E config: {:?}",cfg);
    Ok(cfg)
}


struct Command{
    greeting_receiver_url: String,
    message: GreetingTemplate
}

#[derive(Validate, Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GreetingTemplate {
    to: String,
    #[validate(length(min = 1, max = 20))]
    from: String,
    #[validate(length(min = 1, max = 50))]
    heading: String,
    #[validate(length(min = 1, max = 50))]
    message: String,
}

#[test]
fn try_loading_greeting(){

}