use async_openai::{
    config::OpenAIConfig,
    types::{CreateChatCompletionRequestArgs, Role},
    Client,
};
use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;

pub struct GPTConnector {
    client: Client<OpenAIConfig>,
}

pub struct GPTResponse {
    pub role: Role,
    pub content: String,
}

#[derive(Clone, Serialize, Deserialize)]
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

    pub async fn send_request(&self, messages: Vec<ChatCompletionRequestMessage>) -> Result<GPTResponse, Box<dyn Error>> {
        let api_messages: Vec<async_openai::types::ChatCompletionRequestMessage> = messages
            .into_iter()
            .map(|msg| async_openai::types::ChatCompletionRequestMessage {
                role: msg.role,
                content: Some(msg.content),
                function_call: None,
                name: None
            })
            .collect();
        
        let request_args = CreateChatCompletionRequestArgs::default()
            .max_tokens(512u16)
            .model("gpt-3.5-turbo")
            .messages(api_messages)
            .build()?;
        
        let response_data = self.client.chat().create(request_args).await?;
        
        let message = response_data.choices.get(0).map_or("", |choice| &choice.message.content.as_deref().unwrap_or_default());
        let role = response_data.choices.get(0).map_or(Role::System, |choice| choice.message.role.clone());
        
        Ok(GPTResponse { role, content: message.to_string() })
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
