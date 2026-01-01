use clap::Parser;
use confy::ConfyError;
use log::info;
use thiserror::Error;
use crate::api::{load_e2e_config};
use crate::greeting_api_service::GreetingApiClient;

mod api;
mod greeting_api_service;

#[tokio::main]
async fn main() -> Result<(), E2EError>{
    let args = CliArgs::parse();
    let cfg = load_e2e_config(&args.config_path)?;
    info!("Loaded E2E config: {:?}", cfg);

    let greeting_api_client = GreetingApiClient::new_client(cfg.greeting_api_url);
    let last_log_entry = greeting_api_client.get_last_log_entry().await?;
    info!("The latest loggentry from Greeting-api is: {:?}",last_log_entry);
    Ok(())
//     load config and testspec
//         number of messages
//         number of clients
//     get latest log entry
//     generate greetings
//     send greetings
//     verify all greetings are stored and accessible via API checks
}

/// Runs e2e test for greeting-solution.
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub(crate) struct CliArgs {
    /// Path to configfile. If missing, a template file with default values is created.
    #[arg(short = 'c', long = "config")]
    pub config_path: String,
}


#[derive(Error, Debug)]
enum E2EError{
    #[error("E2E config error: {0}")]
    ConfigError(#[from] ConfyError),
    #[error("Client error: {0}")]
    ClientError(#[from] reqwest::Error),
}
