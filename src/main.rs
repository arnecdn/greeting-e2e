use clap::Parser;
use confy::ConfyError;
use tracing::info;
use thiserror::Error;
use tracing::Level;
use crate::api::{load_e2e_config};
use crate::greeting_api::GreetingApiClient;
use crate::greeting_receiver::{generate_random_message, GreetingCmd, GreetingReceiverClient};

mod api;
mod greeting_api;
mod greeting_receiver;

#[tokio::main]
async fn main() -> Result<(), E2EError>{
    // FmtSubscriber logs to stdout by default
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let args = CliArgs::parse();
    let cfg = load_e2e_config(&args.config_path)?;
    info!("Loaded E2E config: {:?}", cfg);

    let greeting_api_client = GreetingApiClient::new_client(cfg.greeting_api_url);
    let offset = if let Some(last_log_entry) = greeting_api_client.get_last_log_entry().await?{
        last_log_entry.id
    }else {
        0
    };

    info!("Log entry id offset: {}", offset);
    let greeting_receiver_client = GreetingReceiverClient::new_client(cfg.greeting_receiver_url);

    let test_messages = (0..cfg.num_iterations).map(|_|generate_random_message())
        .fold(vec![],|mut acc, m| {
            acc.push(m);
        acc
        });

    let mut test_context = vec![];
    for m in test_messages{
        let resp = greeting_receiver_client.send(m.clone()).await?;
        let c = TestContext{
            message: m,
            resp_message_id: resp.message_id
        };
        test_context.push(c);
    }

    let log_entries = greeting_api_client.get_log_entries(offset, cfg.greeting_log_limit).await?;
    info!("Found logentries: {:?}", log_entries);
    Ok(())
//     load config and testspec
//         number of messages
//         number of clients
//     get latest log entry
//     generate greetings
//     send greetings
//     verify all greetings are stored and accessible via API checks
}
struct TestContext{
    message: GreetingCmd,
    resp_message_id: String
}
/// Runs e2e test for greeting-solution.
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub(crate) struct CliArgs {
    /// Path to configfile. If missing, a template file with default values is created.
    #[arg(short = 'c', long = "greeting-e2e-test-config", env="greeting-e2e-test-config")]
    pub config_path: String,
}


#[derive(Error, Debug)]
enum E2EError{
    #[error("E2E config error: {0}")]
    ConfigError(#[from] ConfyError),
    #[error("Client error: {0}")]
    ClientError(#[from] reqwest::Error),
}
