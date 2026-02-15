use crate::api::Generator::InMemory;
use crate::greeting_e2e::E2EError;
use confy::ConfyError;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct E2ETestConfig {
    pub greeting_receiver_url: String,
    pub greeting_api_url: String,
    pub greeting_log_limit: u16,
    pub num_iterations: u16,
    pub message_generator: Generator,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Generator {
    Ollama,
    InMemory,
}

impl Default for E2ETestConfig {
    fn default() -> Self {
        E2ETestConfig {
            greeting_receiver_url: "http://localhost:8080".to_string(),
            greeting_api_url: "http://localhost:8080".to_string(),
            greeting_log_limit: 0,
            num_iterations: 0,
            message_generator: InMemory,
        }
    }
}

impl E2ETestConfig {
    pub fn valiate(&self) -> Result<(), E2EError> {
        if self.num_iterations <= 0 {
            return Err(E2EError::GeneralError("Invalid num_iterations".to_string()));
        }

        Ok(())
    }
}

pub(crate) fn load_e2e_config(path: &str) -> Result<E2ETestConfig, ConfyError> {
    let cfg: E2ETestConfig = confy::load_path(Path::new(&path))?;

    Ok(cfg)
}
