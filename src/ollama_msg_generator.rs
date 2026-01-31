use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::Ollama;
use crate::greeting_e2e::{E2EError, GreetingCmd, MessageGenerator};

pub struct OllamaMessageGenerator;

impl MessageGenerator for OllamaMessageGenerator {
    async fn generate_messages(&self, num_messages: u16) -> Result<Vec<GreetingCmd>, E2EError> {
        let ollama = Ollama::default();
        let model = "codellama".to_string();
        let prompt = format!("Write a JSON aray with with {} objects formatted as the following JSON struct. \
                The values in the elements 'from', 'heading', 'message' and 'to' fields must be randomized.\
                'created' should include datetime in with UTC, 'externalReference' should be a uuid v7.\
                Only include the JSON array in the response.
                'from': '',\
                'heading': '',\
                'message': '',\
                'to': '',\
                'created':'',\
                'externalReference': ''\
            ", num_messages);

        let req = GenerationRequest::new(model, prompt);

        let res = ollama.generate(req).await;

        let msg = match res {
            Ok(v) => v.response,
            Err(e) => return Err(E2EError::GeneralError(e.to_string())),
        };
        println!("{}", msg);
        Ok(serde_json::from_str::<Vec<GreetingCmd>>(&*msg).unwrap())
    }
}