use crate::api::{load_e2e_config, E2ETestConfig};
use crate::greeting_api::{GreetingApiClient, GreetingLoggEntry};
use crate::greeting_receiver::{generate_random_message, GreetingCmd, GreetingReceiverClient};
use clap::Parser;
use confy::ConfyError;
use log::error;
use std::collections::HashMap;
use std::str::FromStr;
use thiserror::Error;
use time::{Duration};
use tokio::time;
use tokio::time::timeout;
use tracing::metadata::ParseLevelError;
use tracing::Level;
use tracing::{debug, info};

mod api;
mod greeting_api;
mod greeting_receiver;

#[tokio::main]
async fn main() -> Result<(), E2EError> {
    // FmtSubscriber logs to stdout by default
    let args = CliArgs::parse();

    tracing_subscriber::fmt()
        .with_max_level(Level::from_str(&args.logging)?)
        .init();

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
            message_id: None,
            greeting_logg_entry: None,
        })
        .fold(HashMap::new(), |mut acc, t| {
            acc.insert(t.external_reference.to_string(), t);
            acc
        });
    info!("Generated {} test tasks", &tasks.len());

    for t in &mut tasks {
        debug!("Sending message: {:?}", &t.1.message.external_reference);
        let resp = greeting_receiver_client.send(t.1.message.clone()).await;
        match resp {
            Ok(v) => t.1.message_id = Some(v.message_id),
            Err(e) => error!(
                "Error sending message.external_reference: {}, {}",
                &t.1.external_reference, e
            ),
        }
    }

    const GREETING_API_RESPONSE_TIMEOUT_SECS: u64 = 10;
    wait_for_new_log_entry(&greeting_api_client, offset, GREETING_API_RESPONSE_TIMEOUT_SECS).await?;

    verify_tasks( greeting_api_client,  &mut tasks, offset, cfg.greeting_log_limit).await?;

    Ok(())
}

async fn verify_tasks(greeting_api_client: GreetingApiClient, mut tasks: &mut HashMap<String, TestTask>, mut current_offset: i64, greeting_log_limit: u16) -> Result<(), E2EError> {
    loop {
        let log_entries = greeting_api_client
            .get_log_entries(current_offset + 1, greeting_log_limit)
            .await?;

        debug!(
            "Found {:?} entries from offset-id: {}",
            &log_entries.len(),
            current_offset
        );
        let temp_offset = log_entries
            .iter()
            .map(|l| l.id)
            .max()
            .or_else(|| Some(current_offset))
            .unwrap();

        for log_entry in log_entries {
            if let Some(entry) = tasks.get_mut(&log_entry.external_reference) {
                entry.greeting_logg_entry = Some(log_entry.clone());
            }
        }

        if tasks.iter().all(|e| e.1.greeting_logg_entry.is_some())
            && tasks.iter().all(|e| e.1.message_id.is_some())
        {
            print_test_result(&mut tasks);
            break;
        }

        current_offset = temp_offset;
    }
    Ok(())
}

async fn wait_for_new_log_entry(greeting_api_client: &GreetingApiClient, current_offset: i64, wait_timeout: u64) -> Result<(), E2EError> {
    timeout(Duration::from_secs(wait_timeout), async {
        while current_offset == greeting_api_client.get_last_log_entry().await?.unwrap().id {
            time::sleep(Duration::from_secs(1)).await;
        }
        Ok::<(), E2EError>(())
    })
        .await
        .map_err(|_| E2EError::TimeoutError("Timeout waiting for new log entries".to_string()))??;
    Ok(())
}

fn print_test_result(tasks: &mut HashMap<String, TestTask>) {
    info!("Successfully verified {} test-tasks", &tasks.len());
    for ctx in tasks {
        let msg = &ctx.1.message;
        let gle = &ctx.1.greeting_logg_entry.as_ref().unwrap();

        debug!(
            "Verified logg-id: {:?}, greeting.created: {:?}, log.created: {:?}",
            gle.id, msg.created, gle.created
        );
    }
}

#[derive(Debug)]
struct TestTask {
    pub external_reference: String,
    pub message: GreetingCmd,
    pub message_id: Option<String>,
    pub greeting_logg_entry: Option<GreetingLoggEntry>,
}
/// Runs e2e test for greeting-solution.
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub(crate) struct CliArgs {
    /// Path to configfile. If missing, a template file with default values is created
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
    #[error("E2E config error: {0}")]
    LoggParseError(#[from] ParseLevelError),
    #[error("Client error: {0}")]
    ClientError(#[from] reqwest::Error),
    #[error("Timeout error: {0}")]
    TimeoutError(String),
}
