use crate::api::load_e2e_config;
use crate::greeting_api::{GreetingApiClient, GreetingLoggEntry};
use crate::greeting_receiver::{generate_random_message, GreetingCmd, GreetingReceiverClient};
use clap::Parser;
use confy::ConfyError;
use std::collections::HashMap;
use std::str::FromStr;
use log::error;
use thiserror::Error;
use tracing::{debug, info};
use tracing::Level;

mod api;
mod greeting_api;
mod greeting_receiver;

#[tokio::main]
async fn main() -> Result<(), E2EError> {
    // FmtSubscriber logs to stdout by default
    let args = CliArgs::parse();

    tracing_subscriber::fmt().with_max_level(Level::from_str(&args.logging).unwrap()).init();

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

    let mut tasks = (0..cfg.num_iterations)
        .map(|_| generate_random_message())
        .map(|m| TestTask {
            external_reference: m.external_reference.to_string(),
            message: m,
            greeting_logg_entry: None,
        })
        .fold(HashMap::new(), |mut acc, t| {
            acc.insert(t.external_reference.to_string(), t);
            acc
        });
    info!("Generated {} test tasks", &tasks.len());

    for t in &tasks {
        greeting_receiver_client.send(t.1.message.clone()).await?;
        debug!("Sent message: {:?}", t.1.message.external_reference);
    }

    let mut current_offset = offset;

    while current_offset == greeting_api_client.get_last_log_entry().await?.unwrap().id {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }

    loop {
        let log_entries = greeting_api_client
            .get_log_entries(current_offset + 1, cfg.greeting_log_limit)
            .await?;

        debug!("Found {:?} entries from offset-id: {}", &log_entries.len(), offset);
        let temp_offset = log_entries.iter().map(|l| l.id).max().or_else(|| Some(offset)).unwrap();

        for log_entry in log_entries {
            if let Some(entry) = tasks.get_mut(&log_entry.external_reference) {
                entry.greeting_logg_entry = Some(log_entry.clone());
            }
        }

        if tasks.iter().all(|e| e.1.greeting_logg_entry.is_some()) {
            print_test_result(&mut tasks);
            break;
        }

        current_offset = temp_offset;
    }

    Ok(())
}


fn print_test_result(tasks: &mut HashMap<String, TestTask>) {
    info!("Verified {} test-tasks",&tasks.len());
    for ctx in tasks {
        let msg = &ctx.1.message;
        let gle = &ctx.1.greeting_logg_entry.as_ref().unwrap();

        debug!("Verified logg-id: {:?}, greeting.created: {:?}, log.created: {:?}",
                    gle.id,
                    msg.created,
                    gle.created
                );
    }
}

#[derive(Debug)]
struct TestTask {
    pub external_reference: String,
    pub message: GreetingCmd,
    pub greeting_logg_entry: Option<GreetingLoggEntry>,
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

    /// Enable debug mode
    #[arg(
        short = 'd',
        long = "debug",
        env = "greeting-e2e-test-debug",
        default_value = "info"
    )]
    pub logging: String,
}

#[derive(Error, Debug)]
enum E2EError {
    #[error("E2E config error: {0}")]
    ConfigError(#[from] ConfyError),
    #[error("Client error: {0}")]
    ClientError(#[from] reqwest::Error),
}
