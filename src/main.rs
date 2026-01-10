use crate::api::{load_e2e_config};
use crate::greeting_api::{GreetingApiClient};
use crate::greeting_receiver::{GreetingReceiverClient};
use clap::Parser;
use log::error;
use std::collections::HashMap;
use std::str::FromStr;
use tracing::Level;
use tracing::{debug, info};
use crate::greeting_e2e::{execute_e2e_test, generate_random_message, E2EError, GreetingApi, GreetingReceiver, TestTask};

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


#[cfg(test)]
mod tests {
    use crate::api::E2ETestConfig;
    use crate::greeting_api::GreetingApiClient;
    use crate::greeting_e2e::{execute_e2e_test, generate_random_message, GreetingApi, GreetingCmd, GreetingReceiver, GreetingResponse};
    use crate::greeting_receiver::GreetingReceiverClient;
    use serde_json::json;
    use wiremock::matchers::{body_json, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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