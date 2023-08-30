use crate::errors::GPTConnectorError;
pub use async_openai::types::Role;
use async_openai::types::{
    ChatCompletionRequestMessage, CreateChatCompletionRequest, CreateChatCompletionResponse,
};
use async_openai::{config::OpenAIConfig, Client};
use std::env;

pub struct GPTConnector {
    client: Client<OpenAIConfig>,
}

pub struct GPTResponse {
    pub role: Role,
    pub content: String,
}

impl GPTConnector {
    pub fn new() -> Self {
        let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
        let config = OpenAIConfig::new().with_api_key(api_key);
        

let backoff = ExponentialBackoffBuilder::new()
    .with_max_elapsed_time(Some(std::time::Duration::from_secs(60)))
    .build();
let client = Client::with_config(config).with_backoff(backoff);

        GPTConnector { client }
    }

    pub async fn send_request(
        &self,
        messages: Vec<String>,
    ) -> Result<CreateChatCompletionResponse, GPTConnectorError> {
        // Using the client variable from the GPTConnector struct

        let mut constructed_messages = Vec::new();
        for message in messages {
            constructed_messages.push(ChatCompletionRequestMessage {
                role: Role::User,
                content: Some(message),
                function_call: None,
                name: None,
            });
        }
        // Construct the request using CreateChatCompletionRequest
        let request = CreateChatCompletionRequest {
            model: "gpt-3.5-turbo".to_string(), // Assuming this as the model you want to use
            messages: constructed_messages,     // Removed the Some() wrapping
            ..Default::default()                // Use default values for other fields
        };

        // Make the API call
        let response_result = self.client.chat().create(request).await;

        match response_result {
            Ok(response) => Ok(response),
            Err(e) => {
                Err(GPTConnectorError::APIError(e.to_string())) // Capturing the API error and converting it to GPTConnectorError::APIError
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_send_request() {
        let connector = GPTConnector::new();
        let response = connector
            .send_request(vec![ChatCompletionRequestMessage {
                role: Role::User,
                content: "Hello, GPT!".to_string(),
            }])
            .await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap().role, Role::Assistant);
    }
}
