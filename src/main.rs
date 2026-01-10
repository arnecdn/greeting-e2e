use crate::api::{load_e2e_config};
use crate::greeting_api::GreetingApiClient;
use crate::greeting_receiver::GreetingReceiverClient;
use clap::Parser;
use log::error;
use std::collections::HashMap;
use std::str::FromStr;
use tracing::Level;
use tracing::{debug, info};
use crate::greeting_e2e::{execute_e2e_test, generate_random_message, E2EError, TestTask};

mod api;
mod greeting_api;
mod greeting_receiver;
mod greeting_e2e;

#[tokio::main]
async fn main() -> Result<(), E2EError> {
    // FmtSubscriber logs to stdout by default
    let args = CliArgs::parse();

    tracing_subscriber::fmt()
        .with_max_level(Level::from_str(&args.logging)?)
        .init();

    let cfg = load_e2e_config(&args.config_path)?;
    info!("Loaded E2E config: {:?}", cfg);

    if cfg.num_iterations <= 0 {
        error!("Invalid num_iterations: {}", cfg.num_iterations);
        return Ok(());
    }
    let greeting_api_client = GreetingApiClient::new_client(cfg.greeting_api_url.to_string());
    let greeting_receiver_client =
        GreetingReceiverClient::new_client(cfg.greeting_receiver_url.to_string());

    let verified_tasks = execute_e2e_test(
        cfg,
        greeting_api_client,
        greeting_receiver_client,
        generate_random_message,
    )
    .await;

    match verified_tasks {
        Ok(v) => print_test_result(&v),
        Err(e) => error!("{}", e),
    }

    Ok(())
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


fn print_test_result(tasks: &HashMap<String, TestTask>) {
    info!("Successfully verified {} test-tasks", &tasks.len());
    for ctx in tasks {
        let msg = &ctx.1.message;

        if let Some(gle) = &ctx.1.greeting_logg_entry.as_ref() {
            debug!(
                "Verified task.external_reference: {}, greeting.created: {:?}, logg-id: {:?}, log.created: {:?}",
                msg.external_reference, msg.created, gle.id, gle.created
            );
        } else {
            debug!(
                "Task not verified task.external_reference: {}, greeting.created: {:?}",
                msg.external_reference, msg.created
            );
        }
    }
}
