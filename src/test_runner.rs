use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::io::Error;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LoggQuery {
    direction: String,
    offset: i64,
    limit: i8,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GreetingLoggEntry {
    id: i64,
    greeting_id: i64,
    created: DateTime<Utc>,
}

async fn get_last_log_entry() -> Result<(), Error> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use reqwest::StatusCode;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn shall_get_last_logg_entry() {
        let mock_server = MockServer::start().await;
        let mock_response_body =
            json!({"created": "2025-12-30T08:01:10.130Z", "greetingId": 1,"id": 1});

        // 3. Configure the mock server to return the response for a specific request
        Mock::given(method("GET"))
            .and(path("/log/last"))
            .respond_with(ResponseTemplate::new(200).set_body_json(mock_response_body))
            .mount(&mock_server)
            .await;

        let client = reqwest::Client::new();
        let response = client
            .get(mock_server.uri() + "/log/last")
            .send()
            .await
            .expect("Failed");

        assert_eq!(response.status(), StatusCode::OK);
    }
}
