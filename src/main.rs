use crate::api::load_e2e_config;
use crate::greeting_api::{GreetingApiClient, GreetingLoggEntry};
use crate::greeting_receiver::{generate_random_message, GreetingCmd, GreetingReceiverClient};
use clap::Parser;
use confy::ConfyError;
use std::collections::HashMap;
use thiserror::Error;
use tracing::info;
use tracing::Level;

mod api;
mod greeting_api;
mod greeting_receiver;

#[tokio::main]
async fn main() -> Result<(), E2EError> {
    // FmtSubscriber logs to stdout by default
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let args = CliArgs::parse();
    let cfg = load_e2e_config(&args.config_path)?;
    info!("Loaded E2E config: {:?}", cfg);

    let greeting_api_client = GreetingApiClient::new_client(cfg.greeting_api_url);
    let offset = if let Some(last_log_entry) = greeting_api_client.get_last_log_entry().await? {
        last_log_entry.id
    } else {
        0
    };

    info!("Log-entry offset-id: {}", offset);
    let greeting_receiver_client = GreetingReceiverClient::new_client(cfg.greeting_receiver_url);

    let test_messages = (0..cfg.num_iterations)
        .map(|_| generate_random_message())
        .fold(vec![], |mut acc, m| {
            acc.push(m);
            acc
        });

    let mut test_contexts = HashMap::new();
    for m in test_messages {
        let resp = greeting_receiver_client.send(m.clone()).await?;
        let c = TestContext {
            message_id: resp.message_id,
            external_reference: m.external_reference.to_string(),
            message: m,
            greeting_logg_entry: None,
        };
        test_contexts.insert(c.external_reference.to_string(), c);
    }
    let mut current_offset = offset;

    while true {
        info!("Checking log entries from offset-id: {:?}", current_offset);
        let mut log_entries = greeting_api_client
            .get_log_entries(current_offset+1, cfg.greeting_log_limit)
            .await?;

        info!("Found {:?} entries from offset-id: {}", &log_entries.len(), offset);
        log_entries.sort_by(|a,b| a.id.cmp(&b.id));
        current_offset = log_entries.iter().map(|l|l.id).max().or_else(||Some(offset)).unwrap();

        for log_entry in log_entries {
            if let Some(entry)= test_contexts.get_mut(&log_entry.external_reference){
                entry.greeting_logg_entry = Some(log_entry.clone());
                info!("Verified logg-id: {:?}", log_entry.id);
            }
        }

        if test_contexts.iter().all(|e| e.1.greeting_logg_entry .is_some()){
            info!("All messages verified on log");
            break;
        }

    }

    Ok(())
    //     load config and testspec
    //         number of messages
    //         number of clients
    //     get latest log entry
    //     generate greetings
    //     send greetings
    //     verify all greetings are stored and accessible via API checks
}
struct TestContext {
    external_reference: String,
    message_id: String,
    message: GreetingCmd,
    greeting_logg_entry: Option<GreetingLoggEntry>,
}
/// Runs e2e test for greeting-solution.
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub(crate) struct CliArgs {
    /// Path to configfile. If missing, a template file with default values is created.
    #[arg(
        short = 'c',
        long = "greeting-e2e-test-config",
        env = "greeting-e2e-test-config"
    )]
    pub config_path: String,
}

#[derive(Error, Debug)]
enum E2EError {
    #[error("E2E config error: {0}")]
    ConfigError(#[from] ConfyError),
    #[error("Client error: {0}")]
    ClientError(#[from] reqwest::Error),
}
