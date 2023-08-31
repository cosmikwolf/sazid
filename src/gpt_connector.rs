use crate::errors::GPTConnectorError;
pub use async_openai::types::Role;
use async_openai::types::{
    ChatCompletionRequestMessage, CreateChatCompletionRequest, CreateChatCompletionResponse,
};
use async_openai::{config::OpenAIConfig, Client};
use config::Config;
use serde::{Deserialize, Serialize};

use backoff::ExponentialBackoffBuilder;
use std::env;
use std::borrow::Cow;
struct ModelsList {
    default: Model<'a>,
    fallback: Model<'a>,
}
#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub struct Model<'a> {
    pub(crate) name: Cow('a str),
    pub(crate) endpoint: Cow('a str),
    pub(crate) token_limit: u32,
}

const GPT3_TURBO: Model = Model {
    name: "gpt-3.5-turbo",
    endpoint: "https://api.openai.com/v1/models/gpt-3.5-turbo",
    token_limit: 4096,
};

const GPT4_TURBO: Model = Model {
    name: "gpt-4",
    endpoint: "https://api.openai.com/v1/models/gpt-4",
    token_limit: 8192,
};

async fn select_model(
    settings: &config::Config,
    client: &Client<OpenAIConfig>,
) -> Result<Model, GPTConnectorError> {
    // Retrieve the list of available models
    let models_response = client.models().list().await;
    match models_response {
        Ok(response) => {
            let model_names: Vec<String> =
                response.data.iter().map(|model| model.id.clone()).collect();
            let available_models = ModelsList {
                default: GPT3_TURBO,
                fallback: GPT4_TURBO,
            };
            // Check if the default model is in the list
            if model_names.contains(&settings.get::<String>("default").unwrap()) {
                Ok(available_models.default)
            }
            // If not, check if the fallback model is in the list
            else if model_names.contains(&settings.get::<String>("fallback").unwrap()) {
                Ok(available_models.fallback)
            }
            // If neither is available, return an error
            else {
                Err(GPTConnectorError::Other(
                    "Neither the default nor the fallback model is accessible.".to_string(),
                ))
            }
        }
        Err(_) => Err(GPTConnectorError::Other(
            "Failed to fetch the list of available models.".to_string(),
        )),
    }
}
#[derive(Clone)]
pub struct GPTConnector {
    client: Client<OpenAIConfig>,
    pub(crate) settings: Config,
    pub(crate) model: Model,
}

pub struct GPTResponse {
    pub role: Role,
    pub content: String,
}

impl GPTConnector {
    pub async fn new(settings: Config) -> Self {
        let api_key: String = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
        let openai_config = OpenAIConfig::new().with_api_key(api_key);
        let backoff = ExponentialBackoffBuilder::new() // Ensure backoff crate is added to Cargo.toml
            .with_max_elapsed_time(Some(std::time::Duration::from_secs(60)))
            .build();
        let client = Client::with_config(openai_config).with_backoff(backoff);
        let model = select_model(&settings, &client).await.unwrap();

        GPTConnector {
            client,
            settings,
            model,
        }
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
