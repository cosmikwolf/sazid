use async_openai::{
    types::{ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs, Role},
    Client, config::OpenAIConfig,
};
use std::error::Error;
use std::env;

pub struct GPTConnector {
    client: Client<OpenAIConfig>,
}

pub struct GPTRequest {
    pub role: Role,
    pub content: String,
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

    pub async fn send_request(&self, message: &str) -> Result<GPTResponse, Box<dyn Error>> {
        let request_args = CreateChatCompletionRequestArgs::default()
            .max_tokens(512u16)
            .model("gpt-3.5-turbo")
            .messages([
                ChatCompletionRequestMessageArgs::default()
                    .role(Role::User)
                    .content(message)
                    .build()?
            ])
            .build()?;
    
        let response_data = self.client.chat().create(request_args).await?;
    
        // Extract the main content from the GPT response for human readability
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
        let response = connector.send_request("Hello, GPT!").await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap().role, Role::Assistant);
    }
}
