use crate::errors::GPTConnectorError;
pub use async_openai::types::Role;
use async_openai::types::{ChatCompletionRequestMessageArgs, CreateChatCompletionResponse};
use async_openai::{config::OpenAIConfig, types::CreateChatCompletionRequestArgs, Client};
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
        let client = Client::with_config(config);
        GPTConnector { client }
    }

    pub async fn send_request(
        &self,
        messages: Vec<String>,
    ) -> Result<CreateChatCompletionResponse, GPTConnectorError> {
        let client = Client::new();

        let mut constructed_messages = Vec::new();
        for message in messages {
            constructed_messages.push(ChatCompletionRequestMessageArgs {
                role: Role::User,
                content: message,
            });
        }

        let request = CreateChatCompletionRequestArgs {
            messages: constructed_messages,
            ..Default::default() // Using default values for other fields
        };

        let response_result = client.chat().create(request).await;

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
            .send_request(vec![ConnectorChatCompletionRequestMessage {
                role: Role::User,
                content: "Hello, GPT!".to_string(),
            }])
            .await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap().role, Role::Assistant);
    }
}
