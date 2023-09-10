use crate::consts::*;
use crate::errors::GPTConnectorError;
use crate::types::*;
pub use async_openai::types::Role;
use async_openai::types::{
    CreateChatCompletionRequest, CreateChatCompletionResponse, ChatCompletionFunctions, CreateEmbeddingResponse, CreateEmbeddingRequestArgs, ChatChoice, ChatCompletionRequestMessage
};
use async_openai::{config::OpenAIConfig, Client};

use backoff::ExponentialBackoffBuilder;
use std::env;

pub fn lookup_model_by_name(model_name: &str) -> Result<Model, GPTConnectorError> {
    let models = vec![GPT3_TURBO.clone(), GPT4.clone()];
    for model in models {
        if model.name == model_name {
            return Ok(model);
        }
    }
    Err(GPTConnectorError::Other("Invalid model".to_string()))
}

impl GPTConnector {
    pub fn new(settings: GPTSettings, include_functions: bool) -> GPTConnector {
        let api_key: String = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
        let openai_config = OpenAIConfig::new().with_api_key(api_key);
        let backoff = ExponentialBackoffBuilder::new() // Ensure backoff crate is added to Cargo.toml
        .with_max_elapsed_time(Some(std::time::Duration::from_secs(60)))
        .build();
        let client = Client::with_config(openai_config).with_backoff(backoff);

        GPTConnector { settings , include_functions, client}
    }

    pub async fn select_model( &self) -> Result<Model, GPTConnectorError> {
        // Retrieve the list of available models
        let models_response = self.client.models().list().await;
        match models_response {
            Ok(response) => {
                let model_names: Vec<String> =
                    response.data.iter().map(|model| model.id.clone()).collect();
                let available_models = ModelsList {
                    default: GPT4.clone(),
                    fallback: GPT3_TURBO.clone(),
                };
                // Check if the default model is in the list
                if model_names.contains(&self.settings.default.name) {
                    Ok(available_models.default)
                }
                // If not, check if the fallback model is in the list
                else if model_names.contains(&self.settings.fallback.name) {
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
    
    
    pub async fn send_request(
        &self,
        request: CreateChatCompletionRequest,
    ) -> Result<CreateChatCompletionResponse, GPTConnectorError> {

        // Make the API call
        let response_result = self.client.chat().create(request.clone()).await;

        match response_result {
            Ok(response) => {
                crate::gpt_commands::handle_chat_response_function_call(request, response.choices.clone());
                Ok(response)
            },
            Err(e) => {
                Err(GPTConnectorError::APIError(e.to_string())) // Capturing the API error and converting it to GPTConnectorError::APIError
            }
        }
    }

    pub fn construct_request(&mut self, content: Vec<String>, previous_messages:Vec<ChatCompletionRequestMessage>, model:Model ) -> CreateChatCompletionRequest {
        // iterate through the vector of ChatCompletionRequestMessage from the interactions stored in session_data as a clone
        let mut messages: Vec<ChatCompletionRequestMessage> = previous_messages;
        for item in content {
            let message = ChatCompletionRequestMessage {
                role: Role::User,
                content: Some(item),
                ..Default::default()
            };
            messages.push(message);
            // new_messages.push(message);
        }

        let functions = match self.include_functions {
            true => Some(crate::gpt_commands::create_chat_completion_function_args(crate::gpt_commands::define_commands())),
            false => None
        };
        // let model = select_model(settings, &client).await.unwrap();
        // return a new CreateChatCompletionRequest
        CreateChatCompletionRequest {
            model: model.name,
            messages,
            functions,
            ..Default::default()
        }
    }


    pub async fn create_embedding_request(
        model: &str,
        input: Vec<&str>,
    ) -> Result<CreateEmbeddingResponse, GPTConnectorError> {
        let client = Client::new();
    
        let request = CreateEmbeddingRequestArgs::default()
            .model(model)
            .input(input)
            .build()?;
    
        let response = client.embeddings().create(request).await?;
    
        Ok(response)
    }


}
#[cfg(test)]
mod tests {

}
