use crate::consts::*;
use crate::errors::GPTConnectorError;
use crate::types::*;
pub use async_openai::types::Role;
use async_openai::types::{
    CreateChatCompletionRequest, CreateChatCompletionResponse, ChatCompletionFunctions, CreateEmbeddingResponse, CreateEmbeddingRequestArgs
};
use async_openai::{config::OpenAIConfig, Client};

use backoff::ExponentialBackoffBuilder;
use std::env;
use std::path::Path;


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

    // create a function that will parse the file Commands.toml at compile time and return a list of ChatCompletionFunctions
    pub fn parse_commands() -> Result<Vec<ChatCompletionFunctions>, GPTConnectorError> {
        let path = Path::new("Commands.toml");
        let commands_file: CommandsFile = toml::from_str(std::fs::read_to_string(path).unwrap().as_str()).unwrap();
        println!("{:?}", commands_file.commands);
        let mut chat_completion_functions: Vec<ChatCompletionFunctions> = Vec::new();
        for command in commands_file.commands {
            let mut parameters: String = String::new();
            for parameter in command.parameters {
                parameters.push_str(toml::to_string(&parameter).unwrap().as_str())
            }

            let chat_completion_function = ChatCompletionFunctions {
                name: command.name,
                description: Some(command.description),
                parameters: serde_json::from_str(parameters.as_str()).unwrap(),
            };
            chat_completion_functions.push(chat_completion_function);
        }
        Ok(chat_completion_functions)
    }


}
#[cfg(test)]
mod tests {

   #[test]
    fn test_parse_commands() {
         let commands = super::GPTConnector::parse_commands().unwrap();
         println!("{:?}", commands);
         assert_eq!(commands.len(), 2);
    } 
}
