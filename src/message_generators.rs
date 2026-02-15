use crate::greeting_e2e::{E2EError, GeneratedMessage, MessageGenerator};
use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::Ollama;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct LocalMessageGenerator;

impl MessageGenerator for LocalMessageGenerator {
    async fn generate_message(&self) -> Result<GeneratedMessage, E2EError> {
        Ok(GeneratedMessage {
            to: String::from("Greeting recipient"),
            from: String::from("Greeting sender"),
            heading: String::from("Greeting heading"),
            message: String::from("Greeting main message"),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaMessageGenerator;

impl MessageGenerator for OllamaMessageGenerator {
    async fn generate_message(&self) -> Result<GeneratedMessage, E2EError> {
        let ollama = Ollama::default();
        let model = "tinyllama".to_string();
        let prompt = "
                Write a JSON object with the following properties:
                 {'to': '', 'from': '','heading': '', 'message': ''}
                The properties have these additional strict constraints:
                Every property must have mimimum 1 character value.
                'from' must be a random name string from minimum 1 and maximum 20 characters,
                'to' must be a random name string from minimum 1 and maximum 20 characters,
                'heading' must be a random heading string from minimum  1 and maximum 20 characters,
                'message' must be a random message string from minimum 1 and maximum 50 characters,
                Properties does not repeat.
                Single JSON object in the response.
                None of the values can contain special characters.
                The JSON must be pretty printed.
                The response must be predictable.
             ";

        let req = GenerationRequest::new(model, prompt);

        let res = ollama.generate(req).await;

        let message_as_json = match res {
            Ok(v) => parse_message(v.response),
            Err(e) => return Err(E2EError::GenerateMessageError(e.to_string())),
        };

        Ok(serde_json::from_str::<GeneratedMessage>(&message_as_json)
            .map_err(|e| E2EError::GenerateMessageError(e.to_string()))?)
    }
}

fn parse_message(generated_message: String) -> String {
    let mut json = false;
    let mut json_map = vec![];

    for c in generated_message.lines() {
        if c.trim().eq("{") {
            json = true;
        } else if c.trim().eq("}") {
            json = false;
            json_map.push(c);
        }
        if json {
            json_map.push(c);
        }
    }
    let m = json_map.concat();
    m
}

#[cfg(test)]
mod tests {
    use crate::greeting_e2e::MessageGenerator;
    use crate::message_generators::OllamaMessageGenerator;
    use futures::future::join_all;

    #[tokio::test]
    async fn should_generate_message() {
        let MESSAGE_COUNT = 10;
        let msg_generator = OllamaMessageGenerator {};

        let awaiting_messages = (0..10)
            .map(|_| msg_generator.generate_message())
            .collect::<Vec<_>>();

        let received = join_all(awaiting_messages).await;
        let result_ok_count = received.iter().filter(|e| e.is_ok()).count();

        assert_eq!(MESSAGE_COUNT, result_ok_count);
    }
}
