use crate::api::{load_e2e_config, E2ETestConfig};
use crate::greeting_api::{GreetingApiClient, GreetingLoggEntry};
use crate::greeting_receiver::{generate_random_message, GreetingCmd, GreetingReceiverClient};
use clap::Parser;
use confy::ConfyError;
use log::error;
use std::collections::HashMap;
use std::str::FromStr;
use thiserror::Error;
use time::Duration;
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

    if cfg.num_iterations <= 0 {
        error!("Invalid num_iterations: {}", cfg.num_iterations);
        return Ok(());
    }
    let greeting_api_client = GreetingApiClient::new_client(cfg.greeting_api_url.to_string());
    let greeting_receiver_client = GreetingReceiverClient::new_client(cfg.greeting_receiver_url.to_string());

    let verified_tasks = execute_e2e_test(cfg, greeting_api_client, greeting_receiver_client).await;

    match verified_tasks {
        Ok(v) => print_test_result(&v),
        Err(e) => error!("{}",e)
    }

    Ok(())
}

async fn execute_e2e_test(cfg: E2ETestConfig, api_client: GreetingApiClient, receiver_client: GreetingReceiverClient) -> Result<HashMap<String, TestTask>, E2EError> {

    let offset = match api_client.get_last_log_entry().await? {
        Some(v) => v.id,
        None => 0,
    };
    info!("Log-entry offset-id: {}", offset);

    let task_list = generate_test_tasks(cfg.num_iterations);
    info!("Generated {} test tasks", &task_list.len());


    let sent_test_tasks = send_messages(task_list, receiver_client).await;
    info!("Sent {} test tasks", &sent_test_tasks.len());

    verify_tasks(api_client, offset, cfg.greeting_log_limit, sent_test_tasks).await
}


fn generate_test_tasks(num_iterations: u16) -> Vec<TestTask> {
    let task_list = (0..num_iterations)
        .map(|_| generate_random_message())
        .map(|m| TestTask {
            external_reference: m.external_reference.to_string(),
            message: m,
            message_id: None,
            greeting_logg_entry: None,
        })
        .fold(vec![], |mut acc, t| {
            acc.push(t);
            acc
        });

    task_list
}


async fn send_messages(task_list: Vec<TestTask>, greeting_receiver_client: GreetingReceiverClient) -> HashMap<String, TestTask> {
    let mut tasks = HashMap::new();

    for task in task_list {
        debug!("Sending message: {:?}", &task.message.external_reference);
        let resp = greeting_receiver_client.send(task.message.clone()).await;

        match resp {
            Ok(v) => {
                let mut performed_task = TestTask::from(task);
                performed_task.message_id = Some(v.message_id.to_string());
                tasks.insert(v.message_id, performed_task);
            }
            Err(e) => error!(
                "Failed sending message.external_reference: {}, error: {:?}",
                task.external_reference, e
            ),
        }
    }
    tasks
}

async fn verify_tasks(
    greeting_api_client: GreetingApiClient,
    offset: i64,
    logg_limit: u16,
    mut tasks: HashMap<String, TestTask>,
) -> Result<HashMap<String, TestTask>, E2EError> {
    const GREETING_API_RESPONSE_TIMEOUT_SECS: u64 = 10;
    let mut current_offset = offset;

    let verified_tasks = timeout(
        Duration::from_secs(GREETING_API_RESPONSE_TIMEOUT_SECS),
        async {
            while tasks.iter().any(|e| e.1.greeting_logg_entry.is_none()) {
                let log_entries_result = greeting_api_client
                    .get_log_entries(current_offset + 1, logg_limit)
                    .await.map_err(|e|E2EError::ClientError(e));


                let log_entries = match log_entries_result {
                    Ok(v) => v,
                    Err(e) => {
                        panic!("Error when verifying tasks: {}", e)
                    }
                };

                if log_entries.is_empty() {
                    time::sleep(Duration::from_secs(1)).await;
                    continue;
                }

                debug!(
                    "Found {:?} entries from offset-id: {}",
                    &log_entries.len(),
                    current_offset
                );

                for log_entry in log_entries {
                    if let Some(entry) = tasks.get_mut(&log_entry.message_id) {
                        entry.greeting_logg_entry = Some(log_entry.clone());
                    }

                    current_offset = log_entry.id;
                }
            }
            Ok::<HashMap<String, TestTask>, E2EError>(tasks)
        },
    )
        .await
        .map_err(|_| E2EError::TimeoutError("Timeout waiting for new log entries".to_string()))??;

    Ok(verified_tasks)
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

#[derive(Debug, Clone)]
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


#[cfg(test)]
mod tests {
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path};
    use crate::api::E2ETestConfig;
    use crate::execute_e2e_test;
    use crate::greeting_api::GreetingApiClient;
    use crate::greeting_receiver::GreetingReceiverClient;

    #[tokio::test]
    async fn should_execute_e2e_for_0_task_successfully() {

        let greeting_receiver_server = MockServer::start().await;
        let greeting_api_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/log/last"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&greeting_api_server)
            .await;

        let test_config = E2ETestConfig{
            greeting_receiver_url: greeting_receiver_server.uri(),
            greeting_api_url: greeting_api_server.uri(),
            greeting_log_limit: 0,
            num_iterations: 0,
        };

        let greeting_api_client = GreetingApiClient::new_client(test_config.greeting_api_url.to_string());
        let greeting_receiver_client = GreetingReceiverClient::new_client(test_config.greeting_receiver_url.to_string());

        let result = execute_e2e_test(test_config, greeting_api_client, greeting_receiver_client).await;

        assert!(result.is_ok());
    }
}
