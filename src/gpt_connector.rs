use async_openai::{
    types::{ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs, Role, Model},
    Client, config::OpenAIConfig,
};
use crate::logger::Logger;
use std::error::Error;
use std::env;

pub struct GPTConnector {
    client: Client<OpenAIConfig>,
    logger: Logger,
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
        use std::path::PathBuf;
let logger = Logger::new(PathBuf::from("."));
        GPTConnector { client, logger }
    }

    pub async fn send_request(&self, model: &str, message: &str) -> Result<GPTResponse, Box<dyn Error>> {
        let request_args = CreateChatCompletionRequestArgs::default()
            .max_tokens(512u16)
            .model(model)
            .messages([
                ChatCompletionRequestMessageArgs::default()
                    .role(Role::User)
                    .content(message)
                    .build()?
            ])
            .build()?;
    
        let response_data = self.client.chat().create(request_args).await?;
        
        // Log the entire response
        self.logger.log(&format!("{:?}", response_data), "response");
    
        // Extract the main content from the GPT response for human readability
        let message = response_data.choices.get(0).map_or("", |choice| &choice.message.content.as_deref().unwrap_or_default());
        let role = response_data.choices.get(0).map_or(Role::System, |choice| choice.message.role.clone());
    
        Ok(GPTResponse { role, content: message.to_string() })
    }    

    fn parse_response(&self, response_data: GPTRequest) -> GPTResponse {
        GPTResponse { role: response_data.role, content: response_data.content }
    }

    pub async fn available_models(&self) -> Result<Vec<Model>, Box<dyn Error>> {
        Ok(self.client.models().list().await?.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_send_request() {
        let connector = GPTConnector::new();
        let response = connector.send_request("gpt-3.5-turbo", "Hello, GPT!").await;
        assert!(response.is_ok());
        assert_eq!(response.unwrap().role, Role::Assistant);
    }

    #[tokio::test]
    async fn test_available_models() {
        let connector = GPTConnector::new();
        let models = connector.available_models().await;
        assert!(models.is_ok());
    }

    #[tokio::test]
    async fn test_parse_response() {
        let connector = GPTConnector::new();
        let request_data = GPTRequest {
            role: Role::Assistant,
            content: "Test response".to_string(),
        };

        let parsed_response = connector.parse_response(request_data);
        assert_eq!(parsed_response.role, Role::Assistant);
        assert_eq!(parsed_response.content, "Test response");
    }
}
