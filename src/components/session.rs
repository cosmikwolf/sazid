use color_eyre::eyre::Result;
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use ratatui::{prelude::*, widgets::*};
use serde_derive::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs, io};
use tokio::sync::mpsc::UnboundedSender;

use async_openai::types::{
  ChatChoice, ChatCompletionRequestMessage, ChatCompletionResponseMessage, CreateChatCompletionRequest,
  CreateChatCompletionResponse, CreateEmbeddingRequestArgs, CreateEmbeddingResponse, Role,
};

use async_openai::{config::OpenAIConfig, Client};
use async_recursion::async_recursion;
use backoff::exponential::ExponentialBackoffBuilder;

use tokio::runtime::Runtime;

use super::{Component, Frame};
use crate::app::{consts::*, errors::*, tools::chunkifier::*, types::ChatMessage, types::*};
use crate::trace_dbg;
use crate::{
  action::Action,
  config::{Config, KeyBindings},
};
use tui_input::{backend::crossterm::EventHandler, Input};

use crate::app::gpt_interface::handle_chat_response_function_call;
use crate::app::gpt_interface::{create_chat_completion_function_args, define_commands};
use crate::app::tools::utils::ensure_directory_exists;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionConfig {
  pub session_id: String,
  pub model: Model,
  pub include_functions: bool,
}

impl Default for SessionConfig {
  fn default() -> Self {
    SessionConfig { session_id: Self::generate_session_id(), model: GPT4.clone(), include_functions: false }
  }
}
impl SessionConfig {
  pub fn generate_session_id() -> String {
    // Get the current time since UNIX_EPOCH in seconds.
    let start = SystemTime::now();
    let since_the_epoch = start.duration_since(UNIX_EPOCH).expect("Time went backwards").as_secs();

    // Introduce a delay of 1 second to ensure unique session IDs even if called rapidly.
    std::thread::sleep(std::time::Duration::from_secs(1));

    // Convert the duration to a String and return.
    since_the_epoch.to_string()
  }
}
#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct Session {
  pub messages: Vec<ChatMessage>,
  pub config: SessionConfig,
  #[serde(skip)]
  pub action_tx: Option<UnboundedSender<Action>>,
}

impl Component for Session {
  fn init(&mut self, area: Rect) -> Result<()> {
    Ok(())
  }
  fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
    trace_dbg!("register_session_action_handler");
    self.action_tx = Some(tx);
    Ok(())
  }
  fn register_config_handler(&mut self, config: Config) -> Result<()> {
    self.config = config.session_config;
    Ok(())
  }
  fn update(&mut self, action: Action) -> Result<Option<Action>> {
    match action {
      Action::SubmitInput(s) => self.submit_input_handler(s),
      Action::ProcessResponse(response) => self.process_response_handler(response),
      _ => (),
    }
    Ok(None)
  }

  //  fn handle_events(&mut self, event: Option<Event>) -> Result<Option<Action>> {
  //    let r = match event {
  //      Some(Action::SubmitInput(s)) => self.process_response_handler(s),
  //      Some(Event::Mouse(mouse_event)) => self.handle_mouse_events(mouse_event)?,
  //      _ => None,
  //    };
  //    Ok(r)
  //  }

  fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
    let rects = Layout::default().constraints([Constraint::Percentage(100), Constraint::Min(3)].as_ref()).split(area);
    for message in self.messages.clone() {
      if let Some(request) = message.request {
        f.render_widget(
          Paragraph::new(request.content.unwrap_or("no content".to_string()))
            .style(Style::default().fg(Color::White))
            .block(
              Block::default()
                .borders(Borders::ALL)
                .border_style(match request.role {
                  Role::User => Style::default().fg(Color::Yellow),
                  Role::Assistant => Style::default().fg(Color::Green),
                  Role::System => Style::default().fg(Color::Blue),
                  Role::Function => Style::default().fg(Color::Red),
                })
                .border_type(BorderType::Rounded),
            ),
          rects[0],
        )
      }
      if let Some(response) = message.response {
        f.render_widget(
          Paragraph::new(response.content.unwrap_or("no content".to_string()))
            .style(Style::default().fg(Color::White))
            .block(
              Block::default()
                .borders(Borders::ALL)
                .border_style(match response.role {
                  Role::User => Style::default().fg(Color::Yellow),
                  Role::Assistant => Style::default().fg(Color::Green),
                  Role::System => Style::default().fg(Color::Blue),
                  Role::Function => Style::default().fg(Color::Red),
                })
                .border_type(BorderType::Rounded),
            ),
          area,
        )
      }
    }
    Ok(())
  }
}

impl Session {
  pub fn new() -> Session {
    // let mut session = Self::default();
    // session.config.session_id = Self::generate_session_id();
    // session
    Self::default()
    // Self {
    //     session_id,
    //     model: GPT4.clone(),
    //     messages: Vec::new(),
    //     include_functions,
    //     action_tx: todo!(),
    // }
  }

  pub fn submit_input_handler(&mut self, input: String) {
    trace_dbg!("submit_input_handler");
    let tx = self.action_tx.clone().unwrap();
    let session_data = self.config.clone();
    let action: Action;
    tokio::spawn(async move {
      tx.send(Action::EnterProcessing).unwrap();
      tx.send(Action::EnterProcessing).unwrap();
      let response = Session::submit_input(input, session_data).await;
      match response {
        Ok(response) => tx.send(Action::ProcessResponse(response)).unwrap(),
        Err(e) => tx.send(Action::Error(format!("Error: {}", e))).unwrap(),
      }
      tx.send(Action::ExitProcessing).unwrap();
    });
  }

  pub fn process_response_handler(&mut self, response: Vec<ChatCompletionResponseMessage>) {
    trace_dbg!("process_response_handler");
    let tx = self.action_tx.clone().unwrap();
    self.messages.append(&mut response.into_iter().map(|x| x.into()).collect());
    tx.send(Action::Update).unwrap();
  }

  pub async fn submit_input(
    input: String,
    config: SessionConfig,
  ) -> std::result::Result<Vec<ChatCompletionResponseMessage>, GPTConnectorError> {
    let new_messages = construct_chat_completion_request_message(&input, &config.model).unwrap();
    let client = create_openai_client();
    let mut response_messages: Vec<ChatCompletionResponseMessage> = Vec::new();
    let response = Session::send_request(new_messages, MAX_FUNCTION_CALL_DEPTH, client, config).await;
    match response {
      Ok(response) => {
        let _ = response.choices.clone().into_iter().map(|choice| response_messages.push(choice.message));
        Ok(response_messages)
      },
      Err(err) => Err(GPTConnectorError::Other("Failed to send reply to function call".to_string())),
    }
  }

  #[async_recursion]
  pub async fn send_request(
    request_messages: Vec<ChatCompletionRequestMessage>,
    recusion_depth: u32,
    client: Client<OpenAIConfig>,
    config: SessionConfig,
  ) -> Result<CreateChatCompletionResponse, GPTConnectorError> {
    // save new messages in session data
    tracing::debug!("entering send_request");
    //  for message in new_messages.clone() {
    //    self.messages.push(message.into());
    //    // self.ui.display_messages();
    //  }
    // append new messages to existing messages from session data to send in request
    // let mut messages: Vec<ChatCompletionRequestMessage> = self.get_all_requests();
    // messages.append(new_messages.clone().as_mut());
    let mut messages: Vec<ChatCompletionResponseMessage> = Vec::new();
    // form and send request
    let request = construct_request(request_messages, config.clone());
    let response_result = client.chat().create(request.clone()).await;

    // process result and recursively send function call response if necessary
    match response_result {
      Ok(response) => {
        // first save the response messages into session data
        for choice in response.choices.clone() {
          messages.push(choice.message);
          // self.ui.display_messages();
        }
        let _ = response.choices.clone().into_iter().map(|choice| {
          messages.push(choice.message)
          // receive_chat_completion_response_message(choice.message.into()
        });

        if recusion_depth == 0 {
          return Ok(response);
        }
        let function_call_response_messages = handle_chat_response_function_call(response.choices.clone());
        match function_call_response_messages {
          Some(function_call_response_messages) => {
            Session::send_request(
              function_call_response_messages,
              recusion_depth - 1,
              client,
              config, // receive_chat_completion_response_message,
            )
            .await
          },
          None => Ok(response),
        }
      },
      Err(err) => {
        println!("Error: {:?}", err);
        Err(GPTConnectorError::Other("Failed to send reply to function call".to_string()))
      },
    }
  }

  pub fn load_session_by_id(session_id: String) -> Session {
    Self::get_session_filepath(session_id.clone());
    let load_result = fs::read_to_string(Self::get_session_filepath(session_id.clone()));
    match load_result {
      Ok(session_data) => return serde_json::from_str(session_data.as_str()).unwrap(),
      Err(_) => {
        println!("Failed to load session data, creating new session");
        Session::new()
      },
    }
  }

  pub fn get_session_filepath(session_id: String) -> PathBuf {
    Path::new(SESSIONS_DIR).join(Self::get_session_filename(session_id))
  }

  pub fn get_session_filename(session_id: String) -> String {
    format!("{}.json", session_id)
  }

  pub fn get_last_session_file_path() -> Option<PathBuf> {
    ensure_directory_exists(SESSIONS_DIR).unwrap();
    let last_session_path = Path::new(SESSIONS_DIR).join("last_session.txt");
    if last_session_path.exists() {
      Some(fs::read_to_string(last_session_path).unwrap().into())
    } else {
      None
    }
  }

  pub fn load_last_session() -> Session {
    let last_session_path = Path::new(SESSIONS_DIR).join("last_session.txt");
    let last_session_id = fs::read_to_string(last_session_path).unwrap();
    Self::load_session_by_id(last_session_id)
  }

  fn save_session(&self) -> io::Result<()> {
    ensure_directory_exists(SESSIONS_DIR).unwrap();
    let session_file_path = Self::get_session_filepath(self.config.session_id.clone());
    let data = serde_json::to_string(&self)?;
    fs::write(session_file_path, data)?;
    self.save_last_session_id();
    Ok(())
  }

  pub fn save_last_session_id(&self) {
    ensure_directory_exists(SESSIONS_DIR).unwrap();
    let last_session_path = Path::new(SESSIONS_DIR).join("last_session.txt");
    fs::write(last_session_path, self.config.session_id.clone()).unwrap();
  }

  pub fn get_all_messages(&self) -> Vec<ChatMessage> {
    self.messages.clone()
  }

  pub fn get_all_requests(&self) -> Vec<ChatCompletionRequestMessage> {
    self
      .messages
      .clone()
      .into_iter()
      .take_while(|x| x.request.is_some())
      .map(|x| x.try_into().unwrap_or_default())
      .collect()
  }

  pub fn get_all_responses(&self) -> Vec<ChatCompletionResponseMessage> {
    self.messages.clone().into_iter().take_while(|x| x.response.is_some()).map(|x| x.try_into().unwrap()).collect()
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
}

pub async fn select_model(settings: &GPTSettings, client: Client<OpenAIConfig>) -> Result<Model, GPTConnectorError> {
  // Retrieve the list of available models
  let models_response = client.models().list().await;
  match models_response {
    Ok(response) => {
      let model_names: Vec<String> = response.data.iter().map(|model| model.id.clone()).collect();
      let available_models = ModelsList { default: GPT4.clone(), fallback: GPT3_TURBO.clone() };
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
        Err(GPTConnectorError::Other("Neither the default nor the fallback model is accessible.".to_string()))
      }
    },
    Err(_) => Err(GPTConnectorError::Other("Failed to fetch the list of available models.".to_string())),
  }
}

pub fn create_openai_client() -> async_openai::Client<OpenAIConfig> {
  let api_key: String = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
  //let api_key: String = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");

  let message = format!("openai api key: {:?}", api_key);
  trace_dbg!(message);
  let openai_config = OpenAIConfig::new().with_api_key(api_key);
  let backoff = ExponentialBackoffBuilder::new() // Ensure backoff crate is added to Cargo.toml
    .with_max_elapsed_time(Some(std::time::Duration::from_secs(60)))
    .build();
  Client::with_config(openai_config).with_backoff(backoff)
}

pub fn construct_chat_completion_request_message(
  content: &str,
  model: &Model,
) -> Result<Vec<ChatCompletionRequestMessage>, GPTConnectorError> {
  let chunks = parse_input(content, CHUNK_TOKEN_LIMIT as usize, model.token_limit as usize).unwrap();

  let messages: Vec<ChatCompletionRequestMessage> = chunks
    .iter()
    .map(|chunk| ChatCompletionRequestMessage { role: Role::User, content: Some(chunk.clone()), ..Default::default() })
    .collect();
  Ok(messages)
}

pub fn construct_request(
  messages: Vec<ChatCompletionRequestMessage>,
  config: SessionConfig, // model: Model,
                         // include_functions: bool,
) -> CreateChatCompletionRequest {
  let functions = match config.include_functions {
    true => Some(create_chat_completion_function_args(define_commands())),
    false => None,
  };
  CreateChatCompletionRequest { model: config.model.name, messages, functions, ..Default::default() }
}

pub async fn create_embedding_request(
  model: &str,
  input: Vec<&str>,
) -> Result<CreateEmbeddingResponse, GPTConnectorError> {
  let client = Client::new();

  let request = CreateEmbeddingRequestArgs::default().model(model).input(input).build()?;

  let response = client.embeddings().create(request).await?;

  Ok(response)
}
