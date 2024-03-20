use async_openai::types::{
  ChatCompletionRequestAssistantMessage, ChatCompletionRequestMessage,
  ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent,
  ChatCompletionTool, CreateChatCompletionRequest, CreateEmbeddingRequestArgs,
  CreateEmbeddingResponse, Role,
};
use color_eyre::owo_colors::OwoColorize;
use crossterm::event::KeyEvent;
use futures::StreamExt;
use futures_util::future::{ready, Ready};
use futures_util::stream::select_all::SelectAll;
use serde::{Deserialize, Serialize};
use std::default::Default;
use std::fs;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::result::Result;
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::Sleep;
use tokio_stream::wrappers::UnboundedReceiverStream;

use async_openai::{config::OpenAIConfig, Client};
use dotenv::dotenv;

use crate::action::Action;
use crate::app::database::data_manager::{
  get_all_embeddings_by_session, search_message_embeddings_by_session,
  DataManager,
};
use crate::app::database::types::QueryableSession;
use crate::app::messages::{
  ChatMessage, MessageContainer, MessageState, ReceiveBuffer,
};
use crate::app::model_tools::tool_call::{get_enabled_tools, handle_tool_call};
use crate::app::request_validation::debug_request_validation;
use crate::app::session_config::SessionConfig;
use crate::app::{consts::*, errors::*, tools::chunkifier::*, types::*};
use crate::trace_dbg;
use backoff::exponential::ExponentialBackoffBuilder;

use crate::app::tools::utils::ensure_directory_exists;

#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
  pub id: i64,
  pub messages: Vec<MessageContainer>,
  pub viewport_width: usize,
  pub config: SessionConfig,
  #[serde(skip)]
  pub openai_config: OpenAIConfig,
  #[serde(skip)]
  pub embeddings_manager: DataManager,
  #[serde(skip)]
  pub action_tx: Option<UnboundedSender<Action>>,
  #[serde(skip)]
  pub action_rx: Option<UnboundedReceiver<Action>>,
  #[serde(skip)]
  pub last_events: Vec<KeyEvent>,
  #[serde(skip)]
  pub render: bool,
  #[serde(skip)]
  pub fn_name: Option<String>,
  #[serde(skip)]
  pub fn_args: Option<String>,
  #[serde(skip)]
  pub request_buffer: Vec<ChatCompletionRequestMessage>,
  #[serde(skip)]
  pub request_buffer_token_count: usize,
  #[serde(skip)]
  pub new_messages: SelectAll<UnboundedReceiverStream<i64>>,
  #[serde(skip)]
  #[serde(default = "default_idle_timer")]
  pub idle_timer: Pin<Box<Sleep>>,
}

fn default_idle_timer() -> Pin<Box<Sleep>> {
  Box::pin(tokio::time::sleep(Duration::from_secs(5)))
}

impl Default for Session {
  fn default() -> Self {
    Session {
      id: rand::random(),
      messages: vec![],
      config: SessionConfig::default(),
      openai_config: OpenAIConfig::default(),
      embeddings_manager: DataManager::default(),
      action_tx: None,
      action_rx: None,
      last_events: vec![],
      viewport_width: 80,
      render: false,
      fn_name: None,
      fn_args: None,
      request_buffer: Vec::new(),
      request_buffer_token_count: 0,
      idle_timer: Box::pin(tokio::time::sleep(Duration::from_secs(5))),
      new_messages: SelectAll::new(),
    }
  }
}

impl From<QueryableSession> for Session {
  fn from(value: QueryableSession) -> Self {
    dotenv().ok();
    let api_key = std::env::var("OPENAI_API_KEY").unwrap();
    let openai_config = OpenAIConfig::new()
      .with_api_key(api_key)
      .with_org_id("org-WagBLu0vLgiuEL12dylmcPFj");
    // let openai_config = OpenAIConfig::new().with_api_base("http://localhost:1234/v1".to_string());
    let idle_timer = Box::pin(tokio::time::sleep(Duration::from_secs(5)));
    Session {
      id: value.id,
      openai_config,
      config: value.config.0,
      idle_timer,
      ..Default::default()
    }
  }
}

impl Session {
  pub fn message_with_unrendered_content(
    &mut self,
  ) -> Ready<Option<(ChatCompletionRequestMessage, i64)>> {
    match self.messages.iter_mut().find(|m| m.has_unrendered_content()) {
      Some(m) => {
        m.unset_has_unrendered_content();
        ready(Some((m.message.clone(), m.message_id)))
      },
      None => ready(None),
    }
  }

  pub fn message_id_with_unrendered_content(&self) -> Ready<Option<i64>> {
    match self.messages.iter().find(|m| m.has_unrendered_content()) {
      Some(message) => ready(Some(message.message_id)),
      None => ready(None),
    }
  }
}

impl Session {
  pub fn new(tx: UnboundedSender<Action>) -> Self {
    Session { action_tx: Some(tx), ..Default::default() }
  }

  pub fn set_system_prompt(&mut self, prompt: &str) {
    let tx = self.action_tx.clone().unwrap();
    self.config.prompt = prompt.to_string();
    tx.send(Action::AddMessage(
      self.id,
      ChatMessage::System(self.config.prompt_message()),
    ))
    .unwrap();
  }

  pub fn update(
    &mut self,
    action: Action,
  ) -> Result<Option<Action>, SazidError> {
    let tx = self.action_tx.clone().unwrap();
    match action {
      Action::Error(e) => {
        log::error!("Action::Error - {:?}", e);
      },
      Action::AddMessage(id, chat_message) => {
        if id == self.id {
          self.add_message(chat_message);
          self.execute_tool_calls();
          self.generate_new_message_embeddings();
        }
      },
      Action::ExecuteCommand(_command) => {
        // tx.send(Action::CommandResult(self.execute_command(command).unwrap())).unwrap();
      },
      Action::SaveSession => {
        // self.save_session().unwrap();
      },
      Action::SubmitInput(s) => {
        self.submit_chat_completion_request(s);
      },
      Action::RequestChatCompletion() => {
        trace_dbg!(level: tracing::Level::INFO, "requesting chat completion");
        self.request_chat_completion(None, tx.clone())
      },
      Action::MessageEmbeddingSuccess(id) => {
        self
          .messages
          .iter_mut()
          .find(|m| m.message_id == id)
          .unwrap()
          .embedding_saved = true;
      },
      Action::SelectModel(model) => self.config.model = model,
      _ => (),
    }
    //self.action_tx.clone().unwrap().send(Action::Render).unwrap();
    Ok(None)
  }

  pub fn update_ui_message(&self, message_id: i64) {
    let tx = self.action_tx.clone().unwrap();
    let message =
      self.messages.iter().find(|m| m.message_id == message_id).unwrap();
    tx.send(Action::MessageUpdate(message.message.clone(), message_id))
      .unwrap();
  }

  pub fn process_pending_actions(&mut self) -> Option<Action> {
    let tx = self.action_tx.clone().unwrap();
    let pending_action = if let Some(action_rx) = &mut self.action_rx {
      if let Ok(action) = &action_rx.try_recv() {
        Some(action.clone())
      } else {
        None
      }
    } else {
      None
    };
    if let Some(pending_action) = pending_action {
      if let Ok(Some(new_action)) = self.update(pending_action.clone()) {
        tx.send(new_action.clone()).unwrap();
        return Some(new_action);
      };
    }
    None
  }

  pub fn add_message(&mut self, message: ChatMessage) {
    match message {
      ChatMessage::User(_) => {
        let mut message = MessageContainer::from(message);
        message.set_current_transaction_flag();
        message.set_has_unrendered_content();
        let id = message.message_id;
        self.messages.push(message);
        self.update_ui_message(id);
      },
      ChatMessage::StreamResponse(new_srvec) => {
        new_srvec.iter().for_each(|sr| {
          if let Some(message) = self.messages.iter_mut().find(|m| {
            // trace_dbg!("message: {:#?}", m);
            m.stream_id == Some(sr.id.clone())
              && matches!(
                m.receive_buffer,
                Some(ReceiveBuffer::StreamResponse(_))
              )
          }) {
            let id = message.message_id;
            message.update_stream_response(sr.clone()).unwrap();
            self.update_ui_message(id);
          } else {
            let mut message: MessageContainer =
              ChatMessage::StreamResponse(vec![sr.clone()]).into();
            message.set_current_transaction_flag();
            message.set_has_unrendered_content();
            let id = message.message_id;
            self.messages.push(message);
            self.update_ui_message(id);
          }
        });
      },
      _ => {
        let mut message: MessageContainer = message.into();
        message.set_current_transaction_flag();
        message.set_has_unrendered_content();
        let id = message.message_id;
        self.messages.push(message);
        self.update_ui_message(id);
      },
      // ChatMessage::Tool(_) => self.messages.push(message.send_in_next_request()),
    };
  }

  pub fn generate_new_message_embeddings(&mut self) {
    let tx = self.action_tx.clone().unwrap();
    self
      .messages
      .iter_mut()
      .filter(|m| {
        m.message_state.contains(MessageState::RECEIVE_COMPLETE)
          && !m.message_state.contains(MessageState::EMBEDDING_SAVED)
      })
      .for_each(|m| {
        tx.send(Action::AddMessageEmbedding(
          self.id,
          m.message_id,
          m.message.clone(),
        ))
        .unwrap();
        m.message_state.set(MessageState::EMBEDDING_SAVED, true);
      })
  }

  pub fn execute_tool_calls(&mut self) {
    let tx = self.action_tx.clone().unwrap();
    self
      .messages
      .iter_mut()
      .filter(|m| {
        m.receive_complete
          && !m.tools_called
          && matches!(m.message, ChatCompletionRequestMessage::Assistant(_))
      })
      .for_each(|m| {
        // trace_dbg!("executing tool calls");
        if let ChatCompletionRequestMessage::Assistant(
          ChatCompletionRequestAssistantMessage {
            tool_calls: Some(tool_calls),
            ..
          },
        ) = &m.message
        {
          tool_calls.iter().for_each(|tc| {
            // let debug_text = format!("calling tool: {:?}", tc);
            // trace_dbg!(level: tracing::Level::INFO, debug_text);
            log::info!("calling tool: {:?}", tc);
            handle_tool_call(tx.clone(), tc, self.config.clone(), self.id);
          });
          m.tools_called = true;
        }
      })
  }

  pub fn add_chunked_chat_completion_request_messages(
    &mut self,
    content: &str,
    name: &str,
    role: Role,
    model: &Model,
  ) -> Result<Vec<ChatMessage>, SazidError> {
    let mut new_messages = Vec::new();
    match parse_input(
      content,
      CHUNK_TOKEN_LIMIT as usize,
      model.token_limit as usize,
    ) {
      Ok(chunks) => {
        chunks.iter().for_each(|chunk| {
          let message = ChatMessage::User(ChatCompletionRequestUserMessage {
            role,
            name: Some(name.into()),
            content: ChatCompletionRequestUserMessageContent::Text(
              chunk.clone(),
            ),
          });
          self.update(Action::AddMessage(self.id, message.clone())).unwrap();
          new_messages.push(message);
        });
        Ok(new_messages)
      },
      Err(e) => Err(SazidError::ChunkifierError(e)),
    }
  }

  fn filter_non_ascii(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii()).collect()
  }

  pub fn submit_chat_completion_request(&mut self, input: String) {
    let tx = self.action_tx.clone().unwrap();
    let config = self.config.clone();
    self
      .messages
      .iter_mut()
      .filter(|m| m.current_transaction_flag)
      .for_each(|m| m.current_transaction_flag = false);
    tx.send(Action::UpdateStatus(Some("submitting input".to_string())))
      .unwrap();
    match self.add_chunked_chat_completion_request_messages(
      Self::filter_non_ascii(&input).as_str(),
      config.user.as_str(),
      Role::User,
      &config.model,
    ) {
      Ok(_) => {
        tx.send(Action::RequestChatCompletion()).unwrap();
      },
      Err(e) => {
        tx.send(Action::Error(format!("Error: {:?}", e))).unwrap();
      },
    }
  }

  pub fn request_chat_completion(
    &mut self,
    input: Option<String>,
    tx: UnboundedSender<Action>,
  ) {
    tx.send(Action::UpdateStatus(Some("Configuring Client".to_string())))
      .unwrap();
    let stream_response = self.config.stream_response;
    let openai_config = self.openai_config.clone();
    let db_url = self.config.database_url.clone();
    let model = self.config.model.clone();
    let embedding_model = self.embeddings_manager.model.clone();
    let user = self.config.user.clone();
    let session_id = self.id;
    let max_tokens = self.config.response_max_tokens;
    let rag = self.config.retrieval_augmentation_message_count;
    let stream = Some(self.config.stream_response);
    let tools = match get_enabled_tools(None)
    // let tools = match get_enabled_tools(Some(self.config.enabled_tools.clone()))
    {
      Ok(tools) => tools,
      Err(e) => {
        log::error!("error getting enabled tools: {:?}", e);
        tx.send(Action::Error(format!("Error: {:?}", e))).unwrap();
        None
      },
    };

    log::info!("tools: {:#?}", tools);

    let new_messages = self
      .messages
      .iter_mut()
      // .filter(|m| m.current_transaction_flag)
      .map(|m| {
        // m.current_transaction_flag = false;
        m.message.clone()
      })
      .collect::<Vec<ChatCompletionRequestMessage>>();
    tx.send(Action::UpdateStatus(Some("Assembling request...".to_string())))
      .unwrap();
    tokio::spawn(async move {
      let mut embeddings_and_messages: Vec<ChatCompletionRequestMessage> =
        Vec::new();

      embeddings_and_messages.extend(match (input, rag) {
        (Some(input), Some(count)) => search_message_embeddings_by_session(
          &db_url,
          session_id,
          &embedding_model,
          &input,
          count,
        )
        .await
        .unwrap(),
        (Some(_), None) => {
          get_all_embeddings_by_session(&db_url, session_id).await.unwrap()
        },
        (None, _) => Vec::new(),
      });

      embeddings_and_messages.extend(new_messages);
      log::info!("embeddings_and_messages: {:#?}", embeddings_and_messages);
      let request = construct_request(
        model.name.clone(),
        embeddings_and_messages,
        stream,
        Some(max_tokens as u16),
        Some(user),
        tools,
      );
      let request_clone = request.clone();
      tx.send(Action::UpdateStatus(Some(
        "Establishing Client Connection".to_string(),
      )))
      .unwrap();
      tx.send(Action::EnterProcessing).unwrap();
      let client = create_openai_client(&openai_config);
      trace_dbg!("client connection established");
      // tx.send(Action::AddMessage(ChatMessage::SazidSystemMessage(format!("Request Token Count: {}", token_count))))
      //   .unwrap();
      match stream_response {
        true => {
          tx.send(Action::UpdateStatus(Some(
            "Sending Request to OpenAI API...".to_string(),
          )))
          .unwrap();
          trace_dbg!("Sending Request to API");
          let mut stream = client.chat().create_stream(request).await.unwrap();
          tx.send(Action::UpdateStatus(Some(
            "Request submitted. Awaiting Response...".to_string(),
          )))
          .unwrap();
          while let Some(response_result) = stream.next().await {
            match response_result {
              Ok(response) => {
                trace_dbg!("Response: {:#?}", response.bright_yellow());
                //tx.send(Action::UpdateStatus(Some(format!("Received responses: {}", count).to_string()))).unwrap();
                tx.send(Action::AddMessage(
                  session_id,
                  ChatMessage::StreamResponse(vec![response]),
                ))
                .unwrap();
                tx.send(Action::Update).unwrap();
              },
              Err(e) => {
                trace_dbg!(
                  "Error: {:#?} -- check https://status.openai.com",
                  e.bright_red()
                );
                debug_request_validation(&request_clone);
                // let reqtext = format!("Request: \n{:#?}", request_clone.clone());
                // trace_dbg!(reqtext);
                trace_dbg!(&request_clone);
                // tx.send(Action::AddMessage(ChatMessage::SazidSystemMessage(reqtext))).unwrap();
                tx.send(Action::Error(format!(
                  "Error: {:?} -- check https://status.openai.com/",
                  e
                )))
                .unwrap();
              },
            }
          }
        },
        false => match client.chat().create(request).await {
          Ok(response) => {
            tx.send(Action::AddMessage(
              session_id,
              ChatMessage::Response(response),
            ))
            .unwrap();
            tx.send(Action::Update).unwrap();
          },
          Err(e) => {
            trace_dbg!("Error: {}", e);
            tx.send(Action::Error(format!(
              "Error: {:#?} -- check https://status.openai.com/",
              e
            )))
            .unwrap();
          },
        },
      };
      tx.send(Action::UpdateStatus(Some("Chat Request Complete".to_string())))
        .unwrap();
      tx.send(Action::SaveSession).unwrap();
      tx.send(Action::ExitProcessing).unwrap();
    });
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

  pub fn select_model(
    model_preference_list: Vec<Model>,
    client: Client<OpenAIConfig>,
  ) {
    trace_dbg!("select model");
    tokio::spawn(async move {
      // Retrieve the list of available models
      let models_response = client.models().list().await;
      match models_response {
        Ok(response) => {
          let available_models: Vec<String> =
            response.data.iter().map(|model| model.id.clone()).collect();
          trace_dbg!("{:?}", available_models);
          // Check if the default model is in the list
          if let Some(preferences) = model_preference_list
            .iter()
            .find(|model| available_models.contains(&model.name))
          {
            Ok(preferences.clone())
          } else {
            Err(SessionManagerError::Other(
              "no preferred models available".to_string(),
            ))
          }
        },
        Err(e) => {
          trace_dbg!("Failed to fetch the list of available models. {:#?}", e);
          Err(SessionManagerError::Other(
            "Failed to fetch the list of available models.".to_string(),
          ))
        },
      }
    });
  }
}

pub fn construct_request(
  model: String,
  messages: Vec<ChatCompletionRequestMessage>,
  stream: Option<bool>,
  max_tokens: Option<u16>,
  user: Option<String>,
  tools: Option<Vec<ChatCompletionTool>>,
) -> CreateChatCompletionRequest {
  // trace_dbg!("request:\n{:#?}", request);
  CreateChatCompletionRequest {
    model,
    messages,
    stream,
    max_tokens,
    user,
    tools,
    ..Default::default()
  }
}
pub fn create_openai_client(
  openai_config: &OpenAIConfig,
) -> async_openai::Client<OpenAIConfig> {
  let backoff =
    ExponentialBackoffBuilder::new() // Ensure backoff crate is added to Cargo.toml
      .with_max_elapsed_time(Some(std::time::Duration::from_secs(60)))
      .build();
  Client::with_config(openai_config.clone()).with_backoff(backoff)
}

pub async fn create_embedding_request(
  model: &str,
  input: Vec<&str>,
) -> Result<CreateEmbeddingResponse, GPTConnectorError> {
  let client = Client::new();
  let request =
    CreateEmbeddingRequestArgs::default().model(model).input(input).build()?;
  let response = client.embeddings().create(request).await?;
  Ok(response)
}
