use confy::ConfyError;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct E2ETestConfig {
    pub greeting_receiver_url: String,
    pub greeting_api_url: String,
    pub num_iterations: i8,
    pub num_clients: i8,
}

impl Default for E2ETestConfig {
    fn default() -> Self {
        E2ETestConfig {
            greeting_receiver_url: "http://localhost:8080".to_string(),
            greeting_api_url: "http://localhost:8080".to_string(),
            num_iterations: 0,
            num_clients: 0,
        }
    }
}

pub(crate) fn load_e2e_config(path: &str) -> Result<E2ETestConfig, ConfyError> {
    let config_path = Path::new(&path);
    let cfg: E2ETestConfig = confy::load_path(config_path)?;

    Ok(cfg)
}
