use crate::api::{load_e2e_config, Generator};
use crate::greeting_api::GreetingApiClient;
use crate::greeting_e2e::{execute_e2e_test, E2EError};
use crate::greeting_receiver::GreetingReceiverClient;
use crate::message_generators::LocalMessageGenerator;
use clap::Parser;
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use log::error;
use message_generators::OllamaMessageGenerator;

mod api;
mod greeting_api;
mod greeting_e2e;
mod greeting_receiver;
mod message_generators;

#[tokio::main]
async fn main() -> Result<(), E2EError> {
    let args = CliArgs::parse();

    let logger =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(args.logging))
            .build();
    let level = logger.filter();
    log::set_max_level(level);

    let multi_progress = MultiProgress::new();
    LogWrapper::new(multi_progress.clone(), logger)
        .try_init()
        .unwrap();

    let cfg = load_e2e_config(&args.config_path)?;

    if cfg.num_iterations <= 0 {
        error!("Invalid num_iterations: {}", cfg.num_iterations);
        return Err(E2EError::GeneralError("Invalid num_iterations".to_string()));
    }

    let greeting_api_client = GreetingApiClient::new_client(cfg.greeting_api_url.to_string());
    let greeting_receiver_client =
        GreetingReceiverClient::new_client(cfg.greeting_receiver_url.to_string());

    match cfg.message_generator {
        Generator::Ollama => {
            execute_e2e_test(
                multi_progress.clone(),
                cfg,
                greeting_api_client,
                greeting_receiver_client,
                OllamaMessageGenerator {},
            )
            .await?
        }
        Generator::Local => {
            execute_e2e_test(
                multi_progress.clone(),
                cfg,
                greeting_api_client,
                greeting_receiver_client,
                LocalMessageGenerator {},
            )
            .await?
        }
    };

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
        short = 'l',
        long = "log-level",
        env = "greeting-e2e-test-log-level",
        default_value = "info"
    )]
    pub logging: String,
}

#[cfg(test)]
mod tests {
    use crate::api::E2ETestConfig;
    use crate::greeting_api::GreetingApiClient;
    use crate::greeting_e2e::{
        execute_e2e_test, E2EError, GeneratedMessage, GreetingResponse, MessageGenerator,
    };

    use crate::api::Generator::Local;
    use crate::greeting_receiver::GreetingReceiverClient;
    use indicatif::MultiProgress;
    use serde_json::{json, Value};
    use wiremock::matchers::{any, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    struct TestGenerator {
        msg: Value,
    }

    impl MessageGenerator for TestGenerator {
        async fn generate_message(&self) -> Result<GeneratedMessage, E2EError> {
            Ok(serde_json::from_value::<GeneratedMessage>(json!(&self.msg))
                .expect("Could not parse json"))
        }
    }

    #[tokio::test]
    async fn should_execute_e2e_for_0_task_successfully() {
        let greeting_receiver_server = MockServer::start().await;
        let greeting_api_server = MockServer::start().await;
        let msg = json!({
            "created": "2026-01-10T09:35:27.262Z",
            "externalReference": "string",
            "from": "string",
            "heading": "string",
            "message": "string",
            "to": "string"
        });

        let test_generator = TestGenerator { msg };

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
            message_generator: Local,
        };

        let greeting_api_client =
            GreetingApiClient::new_client(test_config.greeting_api_url.to_string());
        let greeting_receiver_client =
            GreetingReceiverClient::new_client(test_config.greeting_receiver_url.to_string());

        let result = execute_e2e_test(
            MultiProgress::default(),
            test_config,
            greeting_api_client,
            greeting_receiver_client,
            test_generator,
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

        let test_generator = TestGenerator { msg: msg.clone() };

        let expected_response = GreetingResponse {
            message_id: "019b92bb-0088-77f1-8b09-5d56dfa72bc4".to_string(),
        };
        let greeting_receiver_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/greeting"))
            .and(any())
            .respond_with(ResponseTemplate::new(200).set_body_json(&expected_response))
            .mount(&greeting_receiver_server)
            .await;

        let test_config = E2ETestConfig {
            greeting_receiver_url: greeting_receiver_server.uri(),
            greeting_api_url: greeting_api_server.uri(),
            greeting_log_limit: 10,
            num_iterations: 1,
            message_generator: Local,
        };

        let greeting_api_client =
            GreetingApiClient::new_client(test_config.greeting_api_url.to_string());
        let greeting_receiver_client =
            GreetingReceiverClient::new_client(test_config.greeting_receiver_url.to_string());

        let result = execute_e2e_test(
            MultiProgress::default(),
            test_config,
            greeting_api_client,
            greeting_receiver_client,
            test_generator,
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
