use crate::greeting_e2e::{E2EError, GeneratedMessage, MessageGenerator};
use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::Ollama;


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

        let msg = match res {
            Ok(v) => v.response,
            Err(e) => return Err(E2EError::GeneralError(e.to_string())),
        };

        let mut json = false;
        let mut json_map = vec![];

        for c in msg.lines() {
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
        Ok(serde_json::from_str::<GeneratedMessage>(&m).map_err(|e| E2EError::GenerateMessageError(e.to_string()))?)
    }
}

#[cfg(test)]
mod tests {
    use log::{error, info};
    use crate::greeting_e2e::MessageGenerator;
    use crate::ollama_msg_generator::OllamaMessageGenerator;

    #[tokio::test]
    async fn should_generate_message() {
        let msg_generator = OllamaMessageGenerator {};
        let awaiting_messages = msg_generator.generate_message();

        let messages = awaiting_messages.await;
        match messages {
            Ok(v) => info!("{:?}",v),
            Err(e) => error!("{:?}",e)
        }
    }
}