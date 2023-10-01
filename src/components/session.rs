use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs, io};
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use serde_derive::{Serialize, Deserialize};
use tokio::sync::mpsc::UnboundedSender;
use super::Component;
use color_eyre::eyre::Result;
use crate::{
  action::Action,
  tui::{Event, Frame},
};

use async_openai::types::{
    ChatChoice, ChatCompletionRequestMessage, ChatCompletionResponseMessage,
    CreateChatCompletionRequest, CreateChatCompletionResponse, CreateEmbeddingRequestArgs,
    CreateEmbeddingResponse, Role,
};
use async_openai::{config::OpenAIConfig, Client};
use async_recursion::async_recursion;
use backoff::exponential::ExponentialBackoffBuilder;

use crate::tools::chunkifier::*;
use crate::consts::*;
use crate::errors::*;
use crate::types::ChatMessage;
use crate::types::*;
use crate::components::ui::UI;
use tokio::runtime::Runtime;

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct Session {
    pub session_id: String,
    pub model: Model,
    pub messages: Vec<ChatMessage>,
    pub include_functions: bool,
    #[serde(skip)]
    pub action_tx: Option<UnboundedSender<Action>>,
}

impl Component for Session {
    #[allow(unused_variables)]
    fn init(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
      Ok(())
    }

    fn handle_events(&mut self, event: Option<Event>) -> Option<Action> {
      match event {
        Some(Event::Key(key_event)) => self.handle_key_events(key_event),
        Some(Event::Mouse(mouse_event)) => self.handle_mouse_events(mouse_event),
        _ => None,
      }
    }

    #[allow(unused_variables)]
    fn handle_key_events(&mut self, key: KeyEvent) -> Option<Action> {
      None
    }
    
    #[allow(unused_variables)]
    fn handle_mouse_events(&mut self, mouse: MouseEvent) -> Option<Action> {
      None
    }

    #[allow(unused_variables)]
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::CompleteInput(s) => tokio::spawn(async move {self.submit_input(&s).await;}),
            Action::Quit => todo!(),
            Action::Tick => todo!(),
            Action::Render => todo!(),
            Action::Resume => todo!(),
            Action::Suspend => todo!(),
            Action::Resize(_, _) => todo!(),
            Action::ToggleShowHelp => todo!(),
            Action::ScheduleIncrement => todo!(),
            Action::ScheduleDecrement => todo!(),
            Action::Increment(_) => todo!(),
            Action::Decrement(_) => todo!(),
            Action::EnterNormal => todo!(),
            Action::EnterInsert => todo!(),
            Action::EnterProcessing => todo!(),
            Action::ExitProcessing => todo!(),
            Action::Update => todo!(),
            Action::Error(_) => todo!(),
            Action::GPTResponse(_) => todo!(),
        };
      Ok(None)
    }
    fn draw(&mut self, f: &mut Frame<'_>, rect: Rect) {

    }
  }

impl Session {
    pub fn new(include_functions: bool) -> Session {
        let session_id = Self::generate_session_id();
        Self::default()
        // Self {
        //     session_id,
        //     model: GPT4.clone(),
        //     messages: Vec::new(),
        //     include_functions,
        //     action_tx: todo!(),
        // }
    }

    pub fn load_session_by_id(session_id: String, ui: &UI) -> Session {
        Self::get_session_filepath(session_id.clone());
        let load_result = fs::read_to_string(Self::get_session_filepath(session_id.clone()));
        match load_result {
            Ok(session_data) => return serde_json::from_str(session_data.as_str()).unwrap(),
            Err(_) => {
                println!("Failed to load session data, creating new session");
                return Session::new(false);
            }
        };
    }

    pub fn generate_session_id() -> String {
        // Get the current time since UNIX_EPOCH in seconds.
        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        // Introduce a delay of 1 second to ensure unique session IDs even if called rapidly.
        std::thread::sleep(std::time::Duration::from_secs(1));

        // Convert the duration to a String and return.
        since_the_epoch.to_string()
    }

    pub fn get_session_filepath(session_id: String) -> PathBuf {
        Path::new(SESSIONS_DIR).join(Self::get_session_filename(session_id))
    }

    pub fn get_session_filename(session_id: String) -> String {
        format!("{}.json", session_id)
    }

    pub fn get_last_session_file_path() -> Option<PathBuf> {
        crate::utils::ensure_directory_exists(SESSIONS_DIR).unwrap();
        let last_session_path = Path::new(SESSIONS_DIR).join("last_session.txt");
        if last_session_path.exists() {
            Some(fs::read_to_string(last_session_path).unwrap().into())
        } else {
            None
        }
    }

    pub fn load_last_session(ui: &UI) -> Session {
        let last_session_path = Path::new(SESSIONS_DIR).join("last_session.txt");
        let last_session_id = fs::read_to_string(last_session_path).unwrap();
        Self::load_session_by_id(last_session_id, ui)
    }

    fn save_session(&self) -> io::Result<()> {
        crate::utils::ensure_directory_exists(SESSIONS_DIR).unwrap();
        let session_file_path = Self::get_session_filepath(self.session_id.clone());
        let data = serde_json::to_string(&self)?;
        fs::write(session_file_path, data)?;
        self.save_last_session_id();
        Ok(())
    }

    pub fn save_last_session_id(&self) {
        crate::utils::ensure_directory_exists(SESSIONS_DIR).unwrap();
        let last_session_path = Path::new(SESSIONS_DIR).join("last_session.txt");
        fs::write(last_session_path, self.session_id.clone()).unwrap();
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

    // #[tracing::instrument(skip(self))]
    pub async fn submit_input<'a>(
        &mut self,
        input: &String,
    ) {
        let new_messages = construct_user_messages(input, &self.model).unwrap();
        let client = create_openai_client();
        let tx = self.action_tx.clone().unwrap();
        
        let response = self.send_request(
            new_messages,
            MAX_FUNCTION_CALL_DEPTH, 
            client).await;
                match response {
                    Ok(response) => {
                        let _ = response
                        .choices
                        .clone()
                        .into_iter()
                        .map(|choice| self.messages.push(choice.message.into()));
                    self.save_session().unwrap();
                    tx.send(Action::GPTResponse(response)).unwrap();
                }
                Err(err) => {
                    tx.send(Action::Error(format!("Error: {:?}", err))).unwrap();
                    Err(SessionManagerError::Other(
                        "Failed to send reply to function call".to_string(),
                    ));
                }
            }
            Ok(())
    }

    pub fn get_messages_to_display(&mut self) -> Vec<ChatMessage> {
        let mut messages_to_display: Vec<ChatMessage> = Vec::new();
        for mut message in self.messages.clone() {
            if !message.displayed {
                messages_to_display.push(message.clone());
                message.displayed = true;
            }
        }
        messages_to_display
    }

    // #[tracing::instrument(skip(self, client))]
    #[async_recursion]
    pub async fn send_request<'a>(
        &mut self,
        new_messages: Vec<ChatCompletionRequestMessage>,
        recusion_depth: u32,
        client: Client<OpenAIConfig>,
    ) -> Result<CreateChatCompletionResponse, GPTConnectorError> {
        // save new messages in session data
        tracing::debug!("entering send_request");
        for message in new_messages.clone() {
            self.messages.push(message.into());
            // self.ui.display_messages();
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
                    // self.ui.display_messages();
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
                    crate::components::gpt_commands::handle_chat_response_function_call(
                        response.choices.clone(),
                    );
                match function_call_response_messages {
                    Some(function_call_response_messages) => {
                        // println!(
                        //     "Replying with function call response: {:?}",
                        //     function_call_response_messages
                        // );
                        self.send_request(
                            function_call_response_messages,
                            recusion_depth - 1,
                            client
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
    let chunks = parse_input(
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
        true => Some(crate::components::gpt_commands::create_chat_completion_function_args(
            crate::components::gpt_commands::define_commands(),
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
