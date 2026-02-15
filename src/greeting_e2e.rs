use crate::api::E2ETestConfig;
use chrono::{DateTime, Utc};
use confy::ConfyError;
use future::join_all;
use futures::future;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use log::{debug, error};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;
use tokio::time;
use tokio::time::timeout;
use tracing::metadata::ParseLevelError;
use uuid::Uuid;

pub async fn execute_e2e_test<E, F, G>(
    multi_progress: MultiProgress,
    cfg: E2ETestConfig,
    api_client: E,
    receiver_client: F,
    message_generator: G,
) -> Result<HashMap<String, TestTask>, E2EError>
where
    E: GreetingApi,
    F: GreetingReceiver,
    G: MessageGenerator,
{
    let offset = match api_client.get_last_log_entry().await? {
        Some(v) => v.id,
        None => 0,
    };

    let task_list = generate_test_tasks(
        multi_progress.clone(),
        message_generator,
        cfg.num_iterations,
    )
    .await;

    let sent_test_tasks = send_messages(
        multi_progress.clone(),
        receiver_client,
        cfg.num_iterations,
        task_list,
    )
    .await;

    verify_tasks(
        multi_progress,
        api_client,
        offset,
        cfg.greeting_log_limit,
        cfg.num_iterations,
        sent_test_tasks,
    )
    .await
}

async fn generate_test_tasks<G>(
    mp: MultiProgress,
    message_generator: G,
    num_iterations: u16,
) -> Vec<TestTask>
where
    G: MessageGenerator,
{
    let pb = mp.add(ProgressBar::new(num_iterations as u64));

    pb.set_prefix(format!("{:<20}", "Generating messages"));
    pb.set_style(
        ProgressStyle::with_template(&format!("{{prefix:.bold}}▕{{bar:.{}}}▏{{msg}}", "blue"))
            .unwrap()
            .progress_chars("█ "),
    );
    let start_time = std::time::Instant::now();

    let awaiting_messages = (0..num_iterations)
        .map(|_| message_generator.generate_message())
        .collect::<Vec<_>>();

    let generated_messages = join_all(awaiting_messages).await;

    let greeting_cmnds = generated_messages.into_iter().fold(vec![], |mut acc, res| {
        if let Ok(v) = res {
            acc.push(GreetingCmd {
                to: v.to,
                from: v.from,
                external_reference: Uuid::now_v7().to_string(),
                heading: v.heading,
                message: v.message,
                created: Utc::now(),
            });
        }
        acc
    });

    let generated_tasks = greeting_cmnds
        .into_iter()
        .map(|m| TestTask {
            message: m.clone(),
            message_id: None,
            greeting_logg_entry: None,
        })
        .fold(vec![], |mut acc, t| {
            acc.push(t);
            pb.inc(1);
            pb.set_message(format!(
                "{}/{} generated in {:?}",
                pb.position(),
                num_iterations,
                start_time.elapsed()
            ));
            acc
        });

    pb.abandon_with_message(format!(
        "{}/{} generated in {:?}",
        pb.position(),
        num_iterations,
        start_time.elapsed()
    ));
    generated_tasks
}

async fn send_messages<F>(
    mp: MultiProgress,
    greeting_receiver_client: F,
    number_of_test_tasks: u16,
    task_list: Vec<TestTask>,
) -> HashMap<String, TestTask>
where
    F: GreetingReceiver,
{
    let pb = mp.add(ProgressBar::new(number_of_test_tasks as u64));
    pb.set_prefix(format!("{:<20}", "Sending messages"));

    pb.set_style(
        ProgressStyle::with_template(&format!("{{prefix:.bold}}▕{{bar:.{}}}▏{{msg}}", "yellow"))
            .unwrap()
            .progress_chars("█ "),
    );

    let mut sent_tasks = HashMap::new();
    let start_time = std::time::Instant::now();

    for task in task_list {
        let resp = greeting_receiver_client.send(task.message.clone()).await;

        match resp {
            Ok(v) => {
                let mut sent_task = TestTask::from(task);
                sent_task.message_id = Some(v.message_id.to_string());
                sent_tasks.insert(v.message_id, sent_task);

                pb.inc(1);
                pb.set_message(format!(
                    "{}/{} sent in {:?}",
                    pb.position(),
                    number_of_test_tasks,
                    start_time.elapsed()
                ));
            }
            Err(e) => {
                error!("Failed sending message: {:?}", e)
            }
        }
    }

    pb.abandon_with_message(format!(
        "{}/{} sent in {:?}",
        pb.position(),
        number_of_test_tasks,
        start_time.elapsed()
    ));

    sent_tasks
}

async fn verify_tasks<E>(
    mp: MultiProgress,
    greeting_api_client: E,
    offset: i64,
    logg_limit: u16,
    number_of_test_tasks: u16,
    mut tasks: HashMap<String, TestTask>,
) -> Result<HashMap<String, TestTask>, E2EError>
where
    E: GreetingApi,
{
    const GREETING_API_RESPONSE_TIMEOUT_SECS: u64 = 30;
    let mut current_offset = offset;

    let pb = mp.add(ProgressBar::new(number_of_test_tasks as u64));
    pb.set_prefix(format!("{:<20}", "Verifying messages"));

    pb.set_style(
        ProgressStyle::with_template(&format!("{{prefix:.bold}}▕{{bar:.{}}}▏{{msg}}", "green"))
            .unwrap()
            .progress_chars("█  "),
    );
    let start_time = std::time::Instant::now();

    pb.set_message(format!(
        "{}/{} verified in {:?}",
        pb.position(),
        number_of_test_tasks,
        start_time.elapsed()
    ));

    let verified_tasks = timeout(
        Duration::from_secs(GREETING_API_RESPONSE_TIMEOUT_SECS),
        async {
            let mut verified_tasks = HashMap::new();

            while tasks.is_empty() == false {
                let log_entries = greeting_api_client
                    .get_log_entries(current_offset + 1, logg_limit)
                    .await?;

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
                    let message_id = log_entry.message_id.to_string();

                    if let Some(found_test_task_entry) = tasks.remove_entry(&message_id) {
                        let key = found_test_task_entry.0;
                        let mut verified_test_task = found_test_task_entry.1;

                        verified_test_task.greeting_logg_entry = Some(log_entry.clone());
                        verified_tasks.insert(key, verified_test_task);

                        pb.inc(1);
                        pb.set_message(format!(
                            "{}/{} verified in {:?}",
                            pb.position(),
                            number_of_test_tasks,
                            start_time.elapsed()
                        ));
                    }

                    current_offset = log_entry.id;
                }
            }
            pb.abandon_with_message(format!(
                "{}/{} verified in {:?}",
                pb.position(),
                number_of_test_tasks,
                start_time.elapsed()
            ));

            Ok::<HashMap<String, TestTask>, E2EError>(verified_tasks)
        },
    )
    .await
    .map_err(|_| E2EError::TimeoutError("Timeout waiting for new log entries".to_string()))??;

    Ok(verified_tasks)
}

#[derive(Debug, Clone)]
pub(crate) struct TestTask {
    pub message: GreetingCmd,
    pub message_id: Option<String>,
    pub greeting_logg_entry: Option<GreetingLoggEntry>,
}

pub trait MessageGenerator {
    async fn generate_message(&self) -> Result<GeneratedMessage, E2EError>;
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GeneratedMessage {
    pub(crate) to: String,
    pub(crate) from: String,
    pub(crate) heading: String,
    pub(crate) message: String,
}

pub trait GreetingApi {
    async fn get_last_log_entry(&self) -> Result<Option<GreetingLoggEntry>, E2EError>;
    async fn get_log_entries(
        &self,
        offset: i64,
        limit: u16,
    ) -> Result<Vec<GreetingLoggEntry>, E2EError>;
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
    async fn send(&self, greeting: GreetingCmd) -> Result<GreetingResponse, E2EError>;
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
    ClientError(String),
    #[error("Client error: {0}")]
    ClientHttpError(#[from] reqwest::Error),
    #[error("Timeout error: {0}")]
    TimeoutError(String),
    #[error("General error: {0}")]
    GeneralError(String),
    #[error("General error: {0}")]
    GenerateMessageError(String),
}
