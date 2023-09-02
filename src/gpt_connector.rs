use crate::errors::GPTConnectorError;
pub use async_openai::types::Role;
use async_openai::types::{
    CreateChatCompletionRequest, CreateChatCompletionResponse,
};
use async_openai::{config::OpenAIConfig, Client};
use serde::{Deserialize, Serialize};

use backoff::ExponentialBackoffBuilder;
use std::env;
struct ModelsList {
    default: Model,
    fallback: Model,
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Model {
    pub(crate) name: String,
    pub(crate) endpoint: String,
    pub token_limit: u32,
}
lazy_static! {
    pub static ref GPT3_TURBO: Model = Model {
        name: "gpt-3.5-turbo".to_string(),
        endpoint: "https://api.openai.com/v1/completions".to_string(),
        token_limit: 4096,
    };
    pub static ref GPT4: Model = Model {
        name: "gpt-4".to_string(),
        endpoint: "https://api.openai.com/v1/completions".to_string(),
        token_limit: 4096,
    };
}
pub fn lookup_model_by_name(model_name: &str) -> Result<Model, GPTConnectorError> {
    let models = vec![GPT3_TURBO.clone(), GPT4.clone()];
    for model in models {
        if model.name == model_name {
            return Ok(model);
        }
    }
    Err(GPTConnectorError::Other("Invalid model".to_string()))
}
async fn select_model(
    settings: &GPTSettings,
    client: &Client<OpenAIConfig>,
) -> Result<Model, GPTConnectorError> {
    // Retrieve the list of available models
    let models_response = client.models().list().await;
    match models_response {
        Ok(response) => {
            let model_names: Vec<String> =
                response.data.iter().map(|model| model.id.clone()).collect();
            let available_models = ModelsList {
                default: GPT4.clone(),
                // default: GPT3_TURBO.clone(),
                fallback: GPT3_TURBO.clone(),
            };
            // Check if the default model is in the list
            if model_names.contains(&settings.default.name) {
                Ok(available_models.default)
            }
            // If not, check if the fallback model is in the list
            else if model_names.contains(&settings.fallback.name) {
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

#[derive(Debug, Deserialize, Clone)]
pub struct GPTSettings {
    pub default: ModelConfig,
    pub fallback: ModelConfig,
}
#[derive(Debug, Deserialize, Clone)]
pub struct ModelConfig {
    pub name: String,
}
#[derive(Clone)]
pub struct GPTConnector {
    client: Client<OpenAIConfig>,
    pub(crate) model: Model,
}

pub struct GPTResponse {
    pub role: Role,
    pub content: String,
}

impl GPTConnector {
    pub async fn new(settings: &GPTSettings) -> GPTConnector {
        let api_key: String = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
        let openai_config = OpenAIConfig::new().with_api_key(api_key);
        let backoff = ExponentialBackoffBuilder::new() // Ensure backoff crate is added to Cargo.toml
            .with_max_elapsed_time(Some(std::time::Duration::from_secs(60)))
            .build();
        let client = Client::with_config(openai_config).with_backoff(backoff);
        let model = select_model(settings, &client).await.unwrap();

        GPTConnector { client, model }
    }

    pub async fn send_request(
        &self,
        request: CreateChatCompletionRequest 
    ) -> Result<CreateChatCompletionResponse, GPTConnectorError> {
        // Make the API call
        let response_result = self.client.chat().create(request).await;
        
        match response_result {
            Ok(response) => Ok(response),
            Err(e) => {
                Err(GPTConnectorError::APIError(e.to_string())) // Capturing the API error and converting it to GPTConnectorError::APIError
            }
        }
    }
    pub fn set_gpt_model(&mut self, model: Model) {
        self.model = model;
    }

}
#[cfg(test)]
mod tests {

}
