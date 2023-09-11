use std::env;

use async_openai::{Client, config::OpenAIConfig};
use async_openai::types::{
    ChatCompletionRequestMessage, ChatCompletionResponseMessage, CreateChatCompletionRequest,
    CreateChatCompletionResponse, CreateEmbeddingRequestArgs, CreateEmbeddingResponse, Role, ChatChoice,
};
use backoff::exponential::ExponentialBackoffBuilder;
use async_recursion::async_recursion;

use crate::consts::{CHUNK_TOKEN_LIMIT,GPT3_TURBO, GPT4, MAX_FUNCTION_CALL_DEPTH};
use crate::types::ChatMessage;
use crate::errors::*;
use crate::types::*;
use tokio::runtime::{Handle, Runtime};    

impl Session {
    pub fn new(session_id: String, _settings: GPTSettings, include_functions: bool) -> Session {
        Self {
            session_id,
            model: GPT4.clone(),
            messages: Vec::new(),
            include_functions,
        }
    }

    pub fn get_all_messages(&self) -> Vec<ChatMessage> {
        self.messages.clone()
    }

    pub fn get_all_requests(&self) -> Vec<ChatCompletionRequestMessage> {
        self.messages
            .clone()
            .into_iter()
            .take_while(|x| x.request.is_some())
            .map(|x| x.try_into().unwrap_or_default())
            .collect()
    }

    pub fn get_all_responses(&self) -> Vec<ChatCompletionResponseMessage> {
        self.messages
            .clone()
            .into_iter()
            .take_while(|x| x.response.is_some())
            .map(|x| x.try_into().unwrap())
            .collect()
    }

    pub fn submit_input(&mut self, input: &String, rt: &Runtime) -> Result<Vec<ChatChoice>, SessionManagerError> {
        let new_messages = construct_user_messages(input, &self.model).unwrap();
        let client = create_openai_client();
        let response = rt
            .block_on(async {
                self.send_request(new_messages, MAX_FUNCTION_CALL_DEPTH, client)
                    .await
            })
            .unwrap();

        Ok(response.choices)
    }
    
    #[async_recursion]
    pub async fn send_request(
        &mut self,
        new_messages: Vec<ChatCompletionRequestMessage>,
        recusion_depth: u32,
        client: Client<OpenAIConfig>,
    ) -> Result<CreateChatCompletionResponse, GPTConnectorError> {
        // save new messages in session data
        for message in new_messages.clone() {
            self.messages.push(message.into());
        }
        // append new messages to existing messages from session data to send in request
        let mut messages: Vec<ChatCompletionRequestMessage> = self.get_all_requests();
        messages.append(new_messages.clone().as_mut());

        // form and send request 
        let request = construct_request(messages, self.model.clone(), self.include_functions);
        let response_result = client.chat().create(request.clone()).await;
        
        // process result and recursively send function call response if necessary
        match response_result {
            Ok(response) => {
                // first save the response messages into session data
                for choice in response.choices.clone() {
                    self.messages.push(choice.message.into());
                }
                let _ = response
                .choices
                .clone()
                .into_iter()
                .map(|choice| self.messages.push(choice.message.into()));

                if recusion_depth <= 0 {
                    return Ok(response);
                }
                let function_call_response_messages =
                    crate::gpt_commands::handle_chat_response_function_call(
                        response.choices.clone(),
                    );
                match function_call_response_messages {
                    Some(function_call_response_messages) => {
                        println!(
                            "Replying with function call response: {:?}",
                            function_call_response_messages
                        );
                        self.send_request(
                            function_call_response_messages,
                            recusion_depth - 1,
                            client,
                        )
                        .await
                    }
                    None => Ok(response),
                }
            }
            Err(err) => {
                println!("Error: {:?}", err);
                Err(GPTConnectorError::Other(
                    "Failed to send reply to function call".to_string(),
                ))
            }
        }
    }
}

pub async fn select_model(
    settings: &GPTSettings,
    client: Client<OpenAIConfig>,
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

pub fn create_openai_client() -> async_openai::Client<OpenAIConfig> {
    let api_key: String = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
    let openai_config = OpenAIConfig::new().with_api_key(api_key);
    let backoff = ExponentialBackoffBuilder::new() // Ensure backoff crate is added to Cargo.toml
        .with_max_elapsed_time(Some(std::time::Duration::from_secs(60)))
        .build();
    Client::with_config(openai_config).with_backoff(backoff)
}

pub fn construct_user_messages(
    content: &str,
    model: &Model,
) -> Result<Vec<ChatCompletionRequestMessage>, GPTConnectorError> {
    let chunks = Chunkifier::parse_input(
        content,
        CHUNK_TOKEN_LIMIT as usize,
        model.token_limit as usize,
    )
    .unwrap();

    let messages: Vec<ChatCompletionRequestMessage> = chunks
        .iter()
        .map(|chunk| ChatCompletionRequestMessage {
            role: Role::User,
            content: Some(chunk.clone()),
            ..Default::default()
        })
        .collect();
    Ok(messages)
}

pub fn construct_request(
    messages: Vec<ChatCompletionRequestMessage>,
    model: Model,
    include_functions: bool,
) -> CreateChatCompletionRequest {
    let functions = match include_functions {
        true => Some(crate::gpt_commands::create_chat_completion_function_args(
            crate::gpt_commands::define_commands(),
        )),
        false => None,
    };
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
