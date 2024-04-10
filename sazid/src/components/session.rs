use async_openai::types::{
  ChatCompletionRequestAssistantMessage, ChatCompletionRequestMessage,
  ChatCompletionRequestToolMessage, ChatCompletionRequestUserMessage,
  ChatCompletionRequestUserMessageContent, ChatCompletionTool, CreateChatCompletionRequest,
  CreateEmbeddingRequestArgs, CreateEmbeddingResponse, Role,
};
use futures::StreamExt;
use futures_util::future::{ready, Ready};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::default::Default;
use std::fs;
use std::path::{Path, PathBuf};
use std::result::Result;
use tokio::sync::mpsc::UnboundedSender;

use async_openai::{config::OpenAIConfig, Client};
use dotenv::dotenv;

use crate::action::{ChatToolAction, LsiAction, SessionAction, ToolType};
use crate::app::database::data_manager::{
  get_all_embeddings_by_session, search_message_embeddings_by_session,
};
use crate::app::database::types::QueryableSession;
use crate::app::messages::{ChatMessage, MessageContainer, MessageState, ReceiveBuffer};
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
  pub config: SessionConfig,
  pub enabled_tools: Vec<ChatCompletionTool>,
  #[serde(skip)]
  pub openai_config: OpenAIConfig,
  #[serde(skip)]
  pub action_tx: Option<UnboundedSender<SessionAction>>,
}

impl Default for Session {
  fn default() -> Self {
    Session {
      id: rand::random(),
      messages: vec![],
      config: SessionConfig::default(),
      openai_config: OpenAIConfig::default(),
      enabled_tools: vec![],
      action_tx: None,
    }
  }
}

impl From<QueryableSession> for Session {
  fn from(value: QueryableSession) -> Self {
    dotenv().ok();
    let api_key = std::env::var("OPENAI_API_KEY").unwrap();
    let openai_config =
      OpenAIConfig::new().with_api_key(api_key).with_org_id("org-WagBLu0vLgiuEL12dylmcPFj");
    // let openai_config = OpenAIConfig::new().with_api_base("http://localhost:1234/v1".to_string());
    Session { id: value.id, openai_config, config: value.config.0, ..Default::default() }
  }
}

impl Session {
  pub fn save_session(&self, path: PathBuf) -> Result<(), SazidError> {
    let session_json = serde_json::to_string(&self)?;
    fs::write(path, session_json)?;
    Ok(())
  }

  pub fn load_session(&mut self, path: &PathBuf) -> Result<(), SazidError> {
    let tx = self.action_tx.clone().unwrap();
    let session_json = fs::read_to_string(path)?;
    let session: Session = serde_json::from_str(&session_json)?;
    *self = session;
    self.action_tx = Some(tx.clone());
    tx.send(SessionAction::ReloadMessages(
      self.messages.iter().map(|m| (m.timestamp, m.message.clone())).collect(),
    ))
    .unwrap();
    Ok(())
  }

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
  pub fn new(tx: UnboundedSender<SessionAction>, config: Option<SessionConfig>) -> Self {
    let config = config.unwrap_or_default();
    let session = Session { action_tx: Some(tx.clone()), config, ..Default::default() };
    log::info!("Session created: {:?}", session.id);

    if let Some(workspace_params) = session.config.workspace.clone() {
      tx.send(SessionAction::LsiAction(LsiAction::AddWorkspace(workspace_params))).unwrap();
    }

    tx.send(SessionAction::ChatToolAction(ChatToolAction::ToolListRequest(session.id))).unwrap();

    session
  }

  pub fn set_system_prompt(&mut self, prompt: &str) {
    let tx = self.action_tx.clone().unwrap();
    self.config.prompt = prompt.to_string();
    tx.send(SessionAction::AddMessage(self.id, ChatMessage::System(self.config.prompt_message())))
      .unwrap();
  }

  pub fn update(&mut self, action: SessionAction) -> Result<Option<SessionAction>, SazidError> {
    let tx = self.action_tx.clone().unwrap();
    match action {
      SessionAction::Error(e) => {
        log::error!("Action::Error - {:?}", e);
        Ok(None)
      },
      SessionAction::AddMessage(id, chat_message) => {
        if id == self.id {
          self.add_message(chat_message.clone());
          self.execute_tool_calls();
          self.generate_new_message_embeddings();
        }
        if let ChatMessage::Tool(_) = chat_message {
          Ok(Some(SessionAction::RequestChatCompletion()))
        } else {
          Ok(None)
        }
      },
      SessionAction::UpdateToolList(session_id, tool_list) => {
        if session_id == self.id {
          self.enabled_tools = tool_list
        }
        Ok(None)
      },
      SessionAction::ToolCallComplete(ToolType::LsiQuery(lsi_query), content) => {
        log::info!(
          "Tool Call Complete\nsession_id: {}, tool_call_id: {}\ncontent: {}",
          lsi_query.session_id,
          lsi_query.tool_call_id,
          content
        );

        Ok(Some(SessionAction::AddMessage(
          lsi_query.session_id,
          ChatMessage::Tool(ChatCompletionRequestToolMessage {
            role: Role::Tool,
            content,
            tool_call_id: lsi_query.tool_call_id,
          }),
        )))
      },
      SessionAction::ToolCallError(tool_type, content) => match tool_type {
        ToolType::LsiQuery(lsi_query) => Ok(Some(SessionAction::Error(format!(
          "Language Server Interface Error\nsession_id: {}, tool_call_id: {}\nerror: {}",
          lsi_query.session_id, lsi_query.tool_call_id, content
        )))),
        ToolType::Generic(tool_call_id, content) => Ok(Some(SessionAction::Error(format!(
          "Tool Call Error\ntool_call_id: {}\nerror: {}",
          tool_call_id, content
        )))),
      },
      SessionAction::SaveSession => {
        // self.save_session().unwrap();
        Ok(None)
      },
      SessionAction::SubmitInput(s) => {
        self.submit_chat_completion_request(s);
        Ok(None)
      },
      SessionAction::RequestChatCompletion() => {
        trace_dbg!(level: tracing::Level::INFO, "requesting chat completion");
        self.request_chat_completion(None, tx.clone());
        Ok(None)
      },
      SessionAction::MessageEmbeddingSuccess(id) => {
        self.messages.iter_mut().find(|m| m.message_id == id).unwrap().embedding_saved = true;
        Ok(None)
      },
      _ => Ok(None),
    }
    //self.action_tx.clone().unwrap().send(Action::Render).unwrap();
  }

  pub fn update_ui_message(&self, message_id: i64) {
    let tx = self.action_tx.clone().unwrap();
    let message = self.messages.iter().find(|m| m.message_id == message_id).unwrap();
    tx.send(SessionAction::MessageUpdate(message.message.clone(), message_id)).unwrap();
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
              && matches!(m.receive_buffer, Some(ReceiveBuffer::StreamResponse(_)))
          }) {
            // stream response already exists in receive buffer
            // update existing message
            let id = message.message_id;
            message.update_stream_response(sr.clone()).unwrap();
            self.update_ui_message(id);
          } else {
            // stream response does not exist in stream buffer,
            // create new message in receive buffer
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
        tx.send(SessionAction::AddMessageEmbedding(self.id, m.message_id, m.message.clone()))
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
        m.receive_is_complete()
          && !m.tools_called
          && matches!(m.message, ChatCompletionRequestMessage::Assistant(_))
      })
      .for_each(|m| {
        // trace_dbg!("executing tool calls");
        if let ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessage {
          tool_calls: Some(tool_calls),
          ..
        }) = &m.message
        {
          tool_calls.iter().for_each(|tc| {
            tx.send(SessionAction::ChatToolAction(ChatToolAction::CallTool(tc.clone(), self.id)))
              .unwrap();
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
    match parse_input(content, CHUNK_TOKEN_LIMIT as usize, model.token_limit as usize) {
      Ok(chunks) => {
        chunks.iter().for_each(|chunk| {
          let message = ChatMessage::User(ChatCompletionRequestUserMessage {
            role,
            name: Some(name.into()),
            content: ChatCompletionRequestUserMessageContent::Text(chunk.clone()),
          });
          self.update(SessionAction::AddMessage(self.id, message.clone())).unwrap();
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
    tx.send(SessionAction::UpdateStatus(Some("submitting input".to_string()))).unwrap();
    match self.add_chunked_chat_completion_request_messages(
      Self::filter_non_ascii(&input).as_str(),
      config.user.as_str(),
      Role::User,
      &config.model,
    ) {
      Ok(_) => {
        tx.send(SessionAction::RequestChatCompletion()).unwrap();
      },
      Err(e) => {
        tx.send(SessionAction::Error(format!("Error: {:?}", e))).unwrap();
      },
    }
  }

  pub fn request_chat_completion(
    &mut self,
    input: Option<String>,
    tx: UnboundedSender<SessionAction>,
  ) {
    tx.send(SessionAction::UpdateStatus(Some("Configuring Client".to_string()))).unwrap();
    let stream_response = self.config.stream_response;
    let openai_config = self.openai_config.clone();
    let db_url = self.config.database_url.clone();
    let model = self.config.model.clone();
    let user = self.config.user.clone();
    let session_id = self.id;
    let max_tokens = self.config.response_max_tokens;
    let rag = self.config.retrieval_augmentation_message_count;
    let embedding_model = None;
    let stream = Some(self.config.stream_response);
    let tools = self.enabled_tools.clone();

    let messages = self
      .messages
      .iter_mut()
      // .filter(|m| m.current_transaction_flag)
      .map(|m| {
        // m.current_transaction_flag = false;
        m.message.clone()
      })
      .collect::<Vec<ChatCompletionRequestMessage>>();
    tx.send(SessionAction::UpdateStatus(Some("Assembling request...".to_string()))).unwrap();
    tokio::spawn(async move {
      let mut embeddings_and_messages: Vec<ChatCompletionRequestMessage> = Vec::new();

      if let Some(embedding_model) = embedding_model {
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
          (Some(_), None) => get_all_embeddings_by_session(&db_url, session_id).await.unwrap(),
          (None, _) => Vec::new(),
        });
      }

      embeddings_and_messages.extend(messages);
      log::info!("embeddings_and_messages: {:#?}", embeddings_and_messages);
      let request = construct_request(
        model.name.clone(),
        embeddings_and_messages,
        stream,
        Some(max_tokens as u16),
        Some(user),
        Some(tools),
      );
      let request_clone = request.clone();
      tx.send(SessionAction::UpdateStatus(Some("Establishing Client Connection".to_string())))
        .unwrap();
      let client = create_openai_client(&openai_config);
      trace_dbg!("client connection established");
      // tx.send(Action::AddMessage(ChatMessage::SazidSystemMessage(format!("Request Token Count: {}", token_count))))
      //   .unwrap();
      match stream_response {
        true => {
          tx.send(SessionAction::UpdateStatus(Some(
            "Sending Request to OpenAI API...".to_string(),
          )))
          .unwrap();
          trace_dbg!("Sending Request to API");
          let mut stream = client.chat().create_stream(request).await.unwrap();
          tx.send(SessionAction::UpdateStatus(Some(
            "Request submitted. Awaiting Response...".to_string(),
          )))
          .unwrap();
          while let Some(response_result) = stream.next().await {
            match response_result {
              Ok(response) => {
                // log::debug!("Response: {:#?}", response);
                //tx.send(Action::UpdateStatus(Some(format!("Received responses: {}", count).to_string()))).unwrap();
                tx.send(SessionAction::AddMessage(
                  session_id,
                  ChatMessage::StreamResponse(vec![response]),
                ))
                .unwrap();
              },
              Err(e) => {
                log::error!("Error: {:#?} -- check https://status.openai.com", e);
                debug_request_validation(&request_clone);
                // let reqtext = format!("Request: \n{:#?}", request_clone.clone());
                // trace_dbg!(reqtext);
                // log::debug!("{:#?}", &request_clone);
                // let pretty_json = serde_json::to_string_pretty(&request_clone).unwrap().to_string();
                // log::debug!("{}", pretty_json);
                // tx.send(Action::AddMessage(ChatMessage::SazidSystemMessage(reqtext))).unwrap();
                tx.send(SessionAction::Error(format!(
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
            tx.send(SessionAction::AddMessage(session_id, ChatMessage::Response(response)))
              .unwrap();
          },
          Err(e) => {
            trace_dbg!("Error: {}", e);
            tx.send(SessionAction::Error(format!(
              "Error: {:#?} -- check https://status.openai.com/",
              e
            )))
            .unwrap();
          },
        },
      };
      tx.send(SessionAction::UpdateStatus(Some("Chat Request Complete".to_string()))).unwrap();
      tx.send(SessionAction::SaveSession).unwrap();
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

  pub fn select_model(model_preference_list: Vec<Model>, client: Client<OpenAIConfig>) {
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
          if let Some(preferences) =
            model_preference_list.iter().find(|model| available_models.contains(&model.name))
          {
            Ok(preferences.clone())
          } else {
            Err(SessionManagerError::Other("no preferred models available".to_string()))
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
pub fn create_openai_client(openai_config: &OpenAIConfig) -> async_openai::Client<OpenAIConfig> {
  let backoff = ExponentialBackoffBuilder::new() // Ensure backoff crate is added to Cargo.toml
    .with_max_elapsed_time(Some(std::time::Duration::from_secs(60)))
    .build();
  Client::with_config(openai_config.clone()).with_backoff(backoff)
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
