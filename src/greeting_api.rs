use chrono::{DateTime, Utc};
use log::error;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};

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

pub struct GreetingApiClient {
    client: Client,
    url: String,
}

impl GreetingApiClient {
    pub async fn get_last_log_entry(&self) -> Result<Option<GreetingLoggEntry>, reqwest::Error> {
        let response = self
            .client
            .get(format!("{}/log/last", &self.url))
            .send()
            .await?;

        match response.status().as_str() {
            "200" => Ok(Some(response.json::<GreetingLoggEntry>().await?)),
            "204" => Ok(None),
            _ => {
                let status = response.error_for_status_ref().unwrap_err();
                let error_message = response.text().await?;
                error!("GreetingApiClient.get_last_log_entry: {}", error_message);
                Err(status)
            }
        }
    }

    pub async fn get_log_entries(
        &self,
        offset: i64,
        limit: u16,
    ) -> Result<Vec<GreetingLoggEntry>, reqwest::Error> {
        let response = self
            .client
            .get(format!("{}/log", &self.url))
            .query(&[
                ("direction", "forward"),
                ("offset", &offset.to_string()),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await?;

        let status = response.status();

        if status == 200 {
            Ok(response.json::<Vec<GreetingLoggEntry>>().await?)
        }else if status == 204 {
            Ok(vec![])
        } else {
            let status = response.error_for_status_ref().unwrap_err();
            let error_message = response.text().await?;
            error!("GreetingApiClient.get_log_entries: {}", error_message);
            Err(status)
        }
    }

    pub fn new_client(url: String) -> Self {
        Url::parse(&url).expect("Invalid url");

        GreetingApiClient {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build client"),
            url,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::greeting_api::{GreetingApiClient};
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn should_get_last_logg_entry_with_200_respon() {
        let expected_log_entry = json!(
            {"id": 2, "greetingId": 2, "messageId": "019b92bb-0088-77f1-8b09-5d56dfa72bc4", "created": "2026-01-01T20:56:57.414558Z"}
        );

        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/log/last"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&expected_log_entry))
            .mount(&mock_server)
            .await;

        let greeting_api_client = GreetingApiClient::new_client(mock_server.uri());
        let greeting_log_entry = greeting_api_client
            .get_last_log_entry()
            .await
            .expect("Expected log entry");

        assert_eq!(json!(greeting_log_entry.unwrap()), expected_log_entry);
    }

    #[tokio::test]
    async fn should_get_last_logg_entry_with_204_response() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/log/last"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&mock_server)
            .await;

        let greeting_api_client = GreetingApiClient::new_client(mock_server.uri());
        let greeting_log_entry = greeting_api_client
            .get_last_log_entry()
            .await
            .expect("Expected None");

        assert_eq!(greeting_log_entry, None);
    }

    #[tokio::test]
    async fn should_fail_log_last_with_http_5xx() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/log/last"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let greeting_api_client = GreetingApiClient::new_client(mock_server.uri());
        let resp = greeting_api_client.get_last_log_entry().await;

        assert!(resp.is_err())
    }

    #[tokio::test]
    async fn should_get_latest_log_entries() {
        let expected_log_entries = json!([
            {"id": 1, "greetingId": 1, "messageId": "019b92bb-0088-77f1-8b09-5d56dfa72bc4", "created": "2026-01-01T20:00:00.414558Z"},
            {"id": 2, "greetingId": 2, "messageId": "019b92bb-0088-77f1-8b09-5d56dfa72bc5", "created": "2026-01-01T21:00:00.414558Z"}
        ]);

        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/log"))
            .and(query_param("direction", "forward"))
            .and(query_param("offset", "1"))
            .and(query_param("limit", "10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&expected_log_entries))
            .mount(&mock_server)
            .await;

        let greeting_api_client = GreetingApiClient::new_client(mock_server.uri());
        let resp = greeting_api_client
            .get_log_entries(1, 10)
            .await
            .expect("Expected log entries");

        assert_eq!(json!(resp), expected_log_entries);
    }
}
// {
//   "id": 1,
//   "message": {
//     "externalReference": "019b92d2-f012-7222-a099-0b73d5788b30",
//     "messageId": "019b92bb-0088-77f1-8b09-5d56dfa72bc4",
//     "to": "arne",
//     "from": "arne",
//     "heading": "chrismas carg",
//     "message": "Happy christmas",
//     "created": "2026-01-06T10:20:56.210182",
//     "eventsCreated": {
//       "received_greeting": "2026-01-06T09:54:47.560879858"
//     }
//   },
//   "created": "2026-01-06T09:54:47.609662Z"
// }