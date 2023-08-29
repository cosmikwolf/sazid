use async_openai::types::ChatCompletionRequestMessageArgs;
pub use async_openai::types::Role;
use async_openai::{config::OpenAIConfig, types::CreateChatCompletionRequestArgs, Client};
use serde::{Deserialize, Serialize};
use std::env;
use crate::errors::GPTConnectorError;

pub struct GPTConnector {
    client: Client<OpenAIConfig>,
}

pub struct GPTResponse {
    pub role: Role,
    pub content: String,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct ChatCompletionRequestMessage {
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

    pub async fn send_request(&self, text: &str) -> Result<(), GPTConnectorError> {

        // Building the request
        let request = CreateChatCompletionRequestArgs::default()
            .max_tokens(512u16) // This can be adjusted based on your requirements
            .model("gpt-3.5-turbo")
            .messages([
                ChatCompletionRequestMessageArgs::default()
                    .role(Role::System)
                    .content("You are a helpful assistant.")
                    .build()?,
                ChatCompletionRequestMessageArgs::default()
                    .role(Role::User)
                    .content(text)
                    .build()?,
            ])
            .build()?;

        // Sending the request
        let response = self.client.chat().create(request).await?;

        // For now, just printing the response. This can be adjusted to process the response as needed.
        for choice in response.choices {
            println!(
                "{}: Role: {}  Content: {:?}",
                choice.index, choice.message.role, choice.message.content
            );
        }

        Ok(())
    }}
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
