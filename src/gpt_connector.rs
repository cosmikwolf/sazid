use async_openai::types::{ChatCompletionRequestMessageArgs, CreateChatCompletionResponse};
pub use async_openai::types::Role;
use async_openai::{config::OpenAIConfig, types::CreateChatCompletionRequestArgs, Client};
use std::env;
use crate::errors::GPTConnectorError;

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
    
    pub async fn send_request(&self, messages: Vec<String>) -> Result<CreateChatCompletionResponse, GPTConnectorError> {
        let client = Client::new();

        let mut constructed_messages = Vec::new();

        // Construct the initial system message
        constructed_messages.push(
            ChatCompletionRequestMessageArgs::default()
                .role(Role::System)
                .content("You are a helpful assistant.")
                .build()?
        );

        // Construct user messages
        for message in messages {
            constructed_messages.push(
                ChatCompletionRequestMessageArgs::default()
                    .role(Role::User)
                    .content(&message)
                    .build()?
            );
        }

        let request = CreateChatCompletionRequestArgs::default()
            .max_tokens(512u16)
            .model("gpt-3.5-turbo")
            .messages(constructed_messages.clone())
            .build()?;

        let response = client.chat().create(request).await?;


        Ok(response)
    }}
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
