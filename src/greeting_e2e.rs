use crate::api::E2ETestConfig;
use chrono::{DateTime, Utc};
use confy::ConfyError;
use log::error;
use reqwest::Error;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;
use tokio::time;
use tokio::time::timeout;
use tracing::metadata::ParseLevelError;
use tracing::{debug, info};
use uuid::Uuid;

pub async fn execute_e2e_test<E, F, G>(
    cfg: E2ETestConfig,
    api_client: E,
    receiver_client: F,
    message_generator: G,
) -> Result<HashMap<String, TestTask>, E2EError>
where
    E: GreetingApi,
    F: GreetingReceiver,
    G: Fn() -> GreetingCmd,
{
    let offset = match api_client.get_last_log_entry().await? {
        Some(v) => v.id,
        None => 0,
    };
    info!("Log-entry offset-id: {}", offset);

    let task_list = generate_test_tasks(cfg.num_iterations, message_generator);
    info!("Generated {} test tasks", &task_list.len());

    let sent_test_tasks = send_messages(task_list, receiver_client).await;
    info!("Sent {} test tasks", &sent_test_tasks.len());

    verify_tasks(api_client, offset, cfg.greeting_log_limit, sent_test_tasks).await
}

fn generate_test_tasks<G>(num_iterations: u16, message_generator: G) -> Vec<TestTask>
where
    G: Fn() -> GreetingCmd,
{
    (0..num_iterations)
        .map(|_| message_generator())
        .map(|m| TestTask {
            external_reference: m.external_reference.to_string(),
            message: m,
            message_id: None,
            greeting_logg_entry: None,
        })
        .fold(vec![], |mut acc, t| {
            acc.push(t);
            acc
        })
}

pub fn generate_random_message() -> GreetingCmd {
    GreetingCmd {
        to: "arne".to_string(),
        from: "arne".to_string(),
        heading: "chrismas carg".to_string(),
        message: "Happy christmas".to_string(),
        external_reference: Uuid::now_v7().to_string(),
        created: Utc::now(),
    }
}
async fn send_messages<F>(
    task_list: Vec<TestTask>,
    greeting_receiver_client: F,
) -> HashMap<String, TestTask>
where
    F: GreetingReceiver,
{
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

async fn verify_tasks<E>(
    greeting_api_client: E,
    offset: i64,
    logg_limit: u16,
    mut tasks: HashMap<String, TestTask>,
) -> Result<HashMap<String, TestTask>, E2EError>
where
    E: GreetingApi,
{
    const GREETING_API_RESPONSE_TIMEOUT_SECS: u64 = 10;
    let mut current_offset = offset;

    let verified_tasks = timeout(
        Duration::from_secs(GREETING_API_RESPONSE_TIMEOUT_SECS),
        async {
            while tasks.iter().any(|e| e.1.greeting_logg_entry.is_none()) {
                let log_entries_result = greeting_api_client
                    .get_log_entries(current_offset + 1, logg_limit)
                    .await
                    .map_err(|e| E2EError::ClientError(e));

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

#[derive(Debug, Clone)]
pub(crate) struct TestTask {
    pub external_reference: String,
    pub message: GreetingCmd,
    pub message_id: Option<String>,
    pub greeting_logg_entry: Option<GreetingLoggEntry>,
}

pub trait GreetingApi {
    async fn get_last_log_entry(&self) -> Result<Option<GreetingLoggEntry>, reqwest::Error>;
    async fn get_log_entries(
        &self,
        offset: i64,
        limit: u16,
    ) -> Result<Vec<GreetingLoggEntry>, reqwest::Error>;
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LoggQuery {
    direction: String,
    offset: i64,
    limit: i8,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialOrd, PartialEq, Ord, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GreetingLoggEntry {
    pub(crate) id: i64,
    pub(crate) greeting_id: i64,
    pub(crate) message_id: String,
    pub(crate) created: DateTime<Utc>,
}

pub trait GreetingReceiver {
    async fn send(&self, greeting: GreetingCmd) -> Result<GreetingResponse, Error>;
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GreetingCmd {
    pub(crate) external_reference: String,
    pub(crate) to: String,
    pub(crate) from: String,
    pub(crate) heading: String,
    pub(crate) message: String,
    pub(crate) created: DateTime<Utc>,
}
#[derive(Serialize, Deserialize, Debug, PartialOrd, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GreetingResponse {
    pub message_id: String,
}

#[derive(Error, Debug)]
pub enum E2EError {
    #[error("E2E config error: {0}")]
    ConfigError(#[from] ConfyError),
    #[error("E2E config error: {0}")]
    LoggParseError(#[from] ParseLevelError),
    #[error("Client error: {0}")]
    ClientError(#[from] reqwest::Error),
    #[error("Timeout error: {0}")]
    TimeoutError(String),
}
