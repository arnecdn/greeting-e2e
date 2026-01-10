use crate::greeting_e2e::{GreetingCmd, GreetingReceiver, GreetingResponse};
use log::error;
use reqwest::header::{HeaderMap, CONTENT_TYPE};
use reqwest::{Client, Error, Url};

pub struct GreetingReceiverClient {
    client: Client,
    url: String,
}

impl GreetingReceiverClient {
    pub(crate) fn new_client(url: String) -> Self {
        Url::parse(&url).expect("Invalid url");

        GreetingReceiverClient {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build client"),

            url,
        }
    }
}
impl GreetingReceiver for GreetingReceiverClient {
    async fn send(&self, greeting: GreetingCmd) -> Result<GreetingResponse, Error> {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());

        let response = self
            .client
            .post(format!("{}/greeting", &self.url))
            .headers(headers)
            .json(&greeting)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json::<GreetingResponse>().await?)
        } else {
            let status = response.error_for_status_ref().unwrap_err();
            let error_message = response.text().await?;
            error!("{}", error_message);
            Err(status)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::greeting_e2e::GreetingReceiver;
    use crate::greeting_receiver::{GreetingCmd, GreetingReceiverClient, GreetingResponse};
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn should_send_greeting_message_successfully() {
        let greeting_msg: GreetingCmd = serde_json::from_str(
            r#"{
            "created": "2026-01-02T11:44:14.877Z",
            "externalReference": "exteral refernce",
            "from": "arne",
            "heading": "new year",
            "message": "happy new year",
            "to": "bjarne"
        }"#,
        )
        .unwrap();

        let mock_server = MockServer::start().await;

        let expected_response = GreetingResponse {
            message_id: "1".to_string(),
        };
        Mock::given(method("POST"))
            .and(path("/greeting"))
            .and(body_json(&greeting_msg))
            .respond_with(ResponseTemplate::new(200).set_body_json(&expected_response))
            .mount(&mock_server)
            .await;

        let greeting_receiver_client = GreetingReceiverClient::new_client(mock_server.uri());
        let greeting_response = greeting_receiver_client
            .send(greeting_msg)
            .await
            .expect("Expeced logentry");

        assert_eq!(greeting_response, expected_response);
    }
}
