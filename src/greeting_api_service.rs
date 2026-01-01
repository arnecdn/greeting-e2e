use chrono::{DateTime, Utc};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LoggQuery {
    direction: String,
    offset: i64,
    limit: i8,
}
#[derive(Serialize, Deserialize, Clone, Debug, PartialOrd, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GreetingLoggEntry {
    pub(crate) id: i64,
    greeting_id: i64,
    created: DateTime<Utc>,
}

pub struct GreetingApiClient {
    client: Client,
    url: String,
}

impl GreetingApiClient {
    pub async fn get_last_log_entry(&self) -> Result<Option<GreetingLoggEntry>, reqwest::Error> {
        let response = self.client
            .get(format!("{}/log/last", &self.url))
            .send()
            .await?;

        match response.status().as_str() {
            "200" => Ok(Some(response.json::<GreetingLoggEntry>().await?)),
            "204" => Ok(None),
            _ => Err(response.error_for_status().unwrap_err())
        }
    }


    pub fn new_client(url: String) -> Self {
        Url::parse(&url).expect("Invalid url");

        GreetingApiClient {
            client: reqwest::Client::new(),
            url,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::greeting_api_service::{GreetingApiClient, GreetingLoggEntry};
    use chrono::{DateTime, Utc};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn shaould_get_last_logg_entry()  {

        let created_date_time = DateTime::parse_from_rfc3339("2025-12-30T08:01:10.130Z")
            .unwrap().with_timezone(&Utc);

        let last_log_entry = GreetingLoggEntry {
            id: 1234,
            greeting_id: 1234,
            created: created_date_time,
        };
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/log/last"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&last_log_entry))
            .mount(&mock_server)
            .await;

        let greeting_api_client = GreetingApiClient::new_client(mock_server.uri());
        let greeting_log_entry = greeting_api_client.get_last_log_entry().await.expect("Expeced logentry");

        assert_eq!(greeting_log_entry.unwrap(), last_log_entry);
    }

    #[tokio::test]
     async fn shall_fail_with_http_5xx()  {
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
}
