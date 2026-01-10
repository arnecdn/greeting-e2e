use std::collections::HashMap;
use std::time::Duration;
use chrono::{DateTime, Utc};
use confy::ConfyError;
use log::error;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time;
use tokio::time::timeout;
use tracing::{debug, info};
use tracing::metadata::ParseLevelError;
use uuid::Uuid;
use crate::api::E2ETestConfig;
use crate::greeting_api::GreetingApiClient;
use crate::greeting_receiver::GreetingReceiverClient;


pub async fn execute_e2e_test<F>(
    cfg: E2ETestConfig,
    api_client: GreetingApiClient,
    receiver_client: GreetingReceiverClient,
    messsage_generator: F,
) -> Result<HashMap<String, TestTask>, E2EError>
where
    F: Fn() -> GreetingCmd,
{
    let offset = match api_client.get_last_log_entry().await? {
        Some(v) => v.id,
        None => 0,
    };
    info!("Log-entry offset-id: {}", offset);

    let task_list = generate_test_tasks(cfg.num_iterations, messsage_generator);
    info!("Generated {} test tasks", &task_list.len());

    let sent_test_tasks = send_messages(task_list, receiver_client).await;
    info!("Sent {} test tasks", &sent_test_tasks.len());

    verify_tasks(api_client, offset, cfg.greeting_log_limit, sent_test_tasks).await
}

fn generate_test_tasks<F>(num_iterations: u16, messsage_generator: F) -> Vec<TestTask>
where
    F: Fn() -> GreetingCmd,
{
    (0..num_iterations)
        .map(|_| messsage_generator())
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
async fn send_messages(
    task_list: Vec<TestTask>,
    greeting_receiver_client: GreetingReceiverClient,
) -> HashMap<String, TestTask> {
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

#[cfg(test)]
mod tests {
    use crate::api::E2ETestConfig;
    use crate::greeting_api::GreetingApiClient;
    use crate::greeting_receiver::GreetingReceiverClient;
    use serde_json::json;
    use wiremock::matchers::{body_json, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use crate::greeting_e2e::{execute_e2e_test, generate_random_message, GreetingCmd, GreetingResponse};

    #[tokio::test]
    async fn should_execute_e2e_for_0_task_successfully() {
        let greeting_receiver_server = MockServer::start().await;
        let greeting_api_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/log/last"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&greeting_api_server)
            .await;

        let test_config = E2ETestConfig {
            greeting_receiver_url: greeting_receiver_server.uri(),
            greeting_api_url: greeting_api_server.uri(),
            greeting_log_limit: 0,
            num_iterations: 0,
        };

        let greeting_api_client =
            GreetingApiClient::new_client(test_config.greeting_api_url.to_string());
        let greeting_receiver_client =
            GreetingReceiverClient::new_client(test_config.greeting_receiver_url.to_string());

        let result = execute_e2e_test(
            test_config,
            greeting_api_client,
            greeting_receiver_client,
            generate_random_message,
        )
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty())
    }

    #[tokio::test]
    async fn should_execute_e2e_for_1_task_successfully() {
        let greeting_api_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/log/last"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&greeting_api_server)
            .await;

        let expected_log_entries = json!([
            {"id": 1, "greetingId": 1, "messageId": "019b92bb-0088-77f1-8b09-5d56dfa72bc4", "created": "2026-01-01T20:00:00.414558Z"},
        ]);

        Mock::given(method("GET"))
            .and(path("/log"))
            .and(query_param("direction", "forward"))
            .and(query_param("offset", "1"))
            .and(query_param("limit", "10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&expected_log_entries))
            .mount(&greeting_api_server)
            .await;

        let msg = json!({
            "created": "2026-01-10T09:35:27.262Z",
            "externalReference": "string",
            "from": "string",
            "heading": "string",
            "message": "string",
            "to": "string"
        });

        let test_greeting_generator =
            || serde_json::from_value::<GreetingCmd>(msg.clone()).expect("Could not parse json");

        let greeting_msg = test_greeting_generator();

        let expected_response = GreetingResponse {
            message_id: "019b92bb-0088-77f1-8b09-5d56dfa72bc4".to_string(),
        };
        let greeting_receiver_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/greeting"))
            .and(body_json(greeting_msg))
            .respond_with(ResponseTemplate::new(200).set_body_json(&expected_response))
            .mount(&greeting_receiver_server)
            .await;

        let test_config = E2ETestConfig {
            greeting_receiver_url: greeting_receiver_server.uri(),
            greeting_api_url: greeting_api_server.uri(),
            greeting_log_limit: 10,
            num_iterations: 1,
        };

        let greeting_api_client =
            GreetingApiClient::new_client(test_config.greeting_api_url.to_string());
        let greeting_receiver_client =
            GreetingReceiverClient::new_client(test_config.greeting_receiver_url.to_string());

        let result = execute_e2e_test(
            test_config,
            greeting_api_client,
            greeting_receiver_client,
            test_greeting_generator,
        )
            .await;

        let num_verified = result
            .unwrap()
            .iter()
            .filter(|t| t.1.greeting_logg_entry.is_some())
            .count();
        assert_eq!(num_verified, 1);
    }
}
