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
use helix_view::graphics::Rect;
use serde::{Deserialize, Serialize};
use std::default::Default;
use std::fs;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::result::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::Sleep;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tui::buffer::Buffer;

use async_openai::{config::OpenAIConfig, Client};
use dotenv::dotenv;

use crate::action::Action;
use crate::app::database::data_manager::{
  get_all_embeddings_by_session, search_message_embeddings_by_session,
  DataManager,
};
use crate::app::database::types::QueryableSession;
use crate::app::functions::{all_functions, handle_tool_call};
use crate::app::messages::{
  ChatMessage, MessageContainer, MessageState, ReceiveBuffer,
};
use crate::app::request_validation::debug_request_validation;
use crate::app::session_config::SessionConfig;
use crate::app::session_view::SessionView;
use crate::app::{consts::*, errors::*, tools::chunkifier::*, types::*};
use crate::trace_dbg;
use backoff::exponential::ExponentialBackoffBuilder;

use crate::app::gpt_interface::create_chat_completion_tool_args;
use crate::app::tools::utils::ensure_directory_exists;
use crate::components::home::Mode;

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
  pub view: SessionView,
  #[serde(skip)]
  pub action_tx: Option<UnboundedSender<Action>>,
  #[serde(skip)]
  pub action_rx: Option<UnboundedReceiver<Action>>,
  #[serde(skip)]
  pub mode: Mode,
  #[serde(skip)]
  pub last_events: Vec<KeyEvent>,
  // #[serde(skip)]
  // pub vertical_scroll_state: ScrollbarState,
  // #[serde(skip)]
  // pub horizontal_scroll_state: ScrollbarState,
  #[serde(skip)]
  pub vertical_scroll: usize,
  #[serde(skip)]
  pub horizontal_scroll: usize,
  #[serde(skip)]
  pub scroll_max: usize,
  #[serde(skip)]
  pub vertical_content_height: usize,
  #[serde(skip)]
  pub viewport_height: usize,
  #[serde(skip)]
  pub scroll_sticky_end: bool,
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
  pub input_vsize: u16,
  #[serde(skip)]
  pub cursor_coords: Option<(usize, usize)>,
  #[serde(skip)]
  pub select_start_coords: Option<(usize, usize)>,
  #[serde(skip)]
  pub select_end_coords: Option<(usize, usize)>,
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
      id: 0,
      messages: vec![],
      config: SessionConfig::default(),
      openai_config: OpenAIConfig::default(),
      embeddings_manager: DataManager::default(),
      action_tx: None,
      action_rx: None,
      mode: Mode::Normal,
      last_events: vec![],
      // vertical_scroll_state: ScrollbarState::default(),
      view: SessionView::default(),
      // horizontal_scroll_state: ScrollbarState::default(),
      vertical_scroll: 0,
      scroll_max: 0,
      horizontal_scroll: 0,
      vertical_content_height: 0,
      viewport_width: 80,
      viewport_height: 0,
      scroll_sticky_end: true,
      render: false,
      fn_name: None,
      fn_args: None,
      request_buffer: Vec::new(),
      request_buffer_token_count: 0,
      input_vsize: 1,
      cursor_coords: None,
      select_start_coords: None,
      select_end_coords: None,
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
  pub fn new() -> Self {
    Self::default()
  }

  fn init(&mut self, area: Rect) -> Result<(), SazidError> {
    let (action_tx, action_rx) = tokio::sync::mpsc::unbounded_channel();
    self.action_tx = Some(action_tx);
    self.action_rx = Some(action_rx);

    //let model_preference: Vec<Model> = vec![GPT4.clone(), GPT3_TURBO.clone(), WIZARDLM.clone()];)
    //Session::select_model(model_preference, create_openai_client(self.openai_config.clone()));
    trace_dbg!("init session");
    self.config.prompt =
        [
    "- act as a rust programming assistant",
    "- you write full code when requested",
    "- your responses are conscise and terse",
    "- Use the functions available to execute with the user inquiry.",
    "- Provide ==your responses== as markdown formatted text.",
    "- Make sure to properly entabulate any code blocks",
    "- Do not try and execute arbitrary python code.",
    "- Do not try to infer a path to a file, if you have not been provided a path with the root ./, use the file_search function to verify the file path before you execute a function call.",
    "- If the user asks you to create a file, use the create_file function",
    // "- if the user asks you in a any way to modify a file, use the patch_file function",
    "- Before you ask the user a question, consider if this information exist in the context",
    "- Before you respond, consider if your response is applicable to the current query, ",
    "- Before you respond, consider if your response is appropriate to further the intent of the request",
    "- If you require additional information about the codebase, you can use pcre2grep to gather information about the codebase",
    "- When evaluating function tests, make it a priority to determine if the problems exist in the source code, or if the test code itself is not properly designed"].join("\n").to_string();
    // self.config.prompt = "act as a very terse assistant".into();
    self.view.set_window_width(area.width as usize, &mut self.messages);
    let tx = self.action_tx.clone().unwrap();
    tx.send(Action::AddMessage(ChatMessage::System(
      self.config.prompt_message(),
    )))
    .unwrap();
    self.view.post_process_new_messages(&mut self.messages);
    // self.text_area = TextArea::new(self.view.rendered_text.lines().map(|l| l.to_string()).collect());
    self.config.available_functions = all_functions();
    Ok(())
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
      Action::AddMessage(chat_message) => {
        //trace_dbg!(level: tracing::Level::INFO, "adding message to session");
        self.add_message(chat_message);
        self.view.post_process_new_messages(&mut self.messages);
        self.execute_tool_calls();
        self.generate_new_message_embeddings();
      },
      Action::ExecuteCommand(_command) => {
        // tx.send(Action::CommandResult(self.execute_command(command).unwrap())).unwrap();
      },
      Action::SaveSession => {
        // self.save_session().unwrap();
      },
      Action::SubmitInput(s) => {
        self.scroll_sticky_end = true;
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
      Action::Resize(width, _height) => {
        self.view.set_window_width(width.into(), &mut self.messages);
        self.redraw_messages()
      },
      Action::SelectModel(model) => self.config.model = model,
      Action::SetInputVsize(vsize) => {
        self.input_vsize = vsize;
      },
      Action::EnterCommand => {
        self.view.unfocus_textarea();
        self.mode = Mode::Command;
      },
      Action::EnterNormal => {
        self.view.focus_textarea();
        self.mode = Mode::Normal;
      },
      Action::EnterVisual => {
        self.view.unfocus_textarea();
        self.mode = Mode::Visual;
      },
      Action::EnterInsert => {
        self.view.unfocus_textarea();
        self.mode = Mode::Insert;
      },
      Action::EnterProcessing => {
        self.view.unfocus_textarea();
        self.mode = Mode::Processing;
      },
      Action::ExitProcessing => {
        self.view.focus_textarea();
        self.mode = Mode::Normal;
      },
      _ => (),
    }
    //self.action_tx.clone().unwrap().send(Action::Render).unwrap();
    Ok(None)
  }

  pub fn register_action_handler(
    &mut self,
    tx: UnboundedSender<Action>,
  ) -> Result<(), SazidError> {
    trace_dbg!("register_session_action_handler");
    self.action_tx = Some(tx);
    Ok(())
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
  pub fn render_messages(
    &self,
    scroll: usize,
    area: Rect,
    surface: &mut Buffer,
  ) {
    self
      .messages
      .iter()
      .scan(0, |scroll_top, m| {
        let message_height = m.vertical_height(
          self.view.window_width,
          Arc::clone(&self.view.lang_config.load()),
        );

        let message_line_start_index =
          *scroll_top + message_height - self.vertical_scroll;
        let message_line_last_index = self.vertical_scroll
          + self.viewport_height
          - (*scroll_top + message_height);

        *scroll_top += message_height;

        if *scroll_top + message_height < self.vertical_scroll
          || self.vertical_scroll + self.viewport_height < *scroll_top
        {
          None
        } else {
          Some((m, message_line_start_index, message_line_last_index))
        }
      })
      .for_each(|(m, message_line_start_index, message_line_last_index)| {
        // let content = format!("{}", m);
        // let markdown = Markdown::new(
        //   content,
        //   self.view.window_width,
        //   Arc::clone(&self.view.lang_config),
        // );
        //
        // let text = markdown.parse(None)
        //   [message_line_start_index..message_line_last_index]
        //   .to_vec();
        // let par = Paragraph::new(Text::from(text))
        //   .wrap(Wrap { trim: false })
        //   .scroll((scroll as u16, 0));
        // let margin = Margin::all(1);
        // par.render(area.inner(&margin), surface);
      });
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

  // pub fn get_select_coords(&self) -> Option<((usize, usize), (usize, usize))> {
  //   match self.select_start_coords {
  //     Some((x1, y1)) => match self.select_end_coords {
  //       Some((x2, y2)) => Some(((x1, y1), (x2, y2))),
  //       None => None,
  //     },
  //     None => None,
  //   }
  // }

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
            trace_dbg!("calling tool: {:?}", tc);
            handle_tool_call(tx.clone(), tc, self.config.clone());
          });
          m.tools_called = true;
        }
      })
  }

  fn redraw_messages(&mut self) {
    trace_dbg!("redrawing messages");
    self.messages.iter_mut().for_each(|m| {
      m.stylize_complete = false;
    });
    self.view.post_process_new_messages(&mut self.messages);
  }

  pub fn scroll_up(&mut self) -> Result<Option<Action>, SazidError> {
    self.vertical_scroll = self.vertical_scroll.saturating_sub(1);
    self.scroll_sticky_end = false;
    // trace_dbg!(
    //   "next scroll {} content height: {} vertical_viewport_height: {}",
    //   self.vertical_scroll,
    //   self.vertical_content_height,
    //   self.vertical_viewport_height
    // );
    Ok(Some(Action::Update))
  }

  pub fn scroll_down(&mut self) -> Result<Option<Action>, SazidError> {
    self.vertical_scroll =
      self.vertical_scroll.saturating_add(1).min(self.scroll_max);
    // self.vertical_scroll_state =
    //   self.vertical_scroll_state.position(self.vertical_scroll);
    // if self.vertical_scroll_state
    //   == self.vertical_scroll_state.position(self.scroll_max)
    // {
    //   if !self.scroll_sticky_end {
    //     let mut debug_string = String::new();
    //     for (idx, line) in self.view.rendered_text.lines().enumerate() {
    //       debug_string.push_str(format!("{:02}\t", idx).as_str());
    //       debug_string.push_str(line.to_string().as_str());
    //     }
    //   }
    //   self.scroll_sticky_end = true;
    // }
    // trace_dbg!(
    //   "previous scroll {} content height: {} vertical_viewport_height: {}",
    //   self.vertical_scroll,
    //   self.vertical_content_height,
    //   self.vertical_viewport_height
    // );

    Ok(Some(Action::Update))
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
          self.update(Action::AddMessage(message.clone())).unwrap();
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

    let tools = match self.config.available_functions.is_empty() {
      true => None,
      false => Some(create_chat_completion_tool_args(
        self.config.available_functions.iter().map(|f| f.into()).collect(),
      )),
    };

    let new_messages = self
      .messages
      .iter_mut()
      .filter(|m| m.current_transaction_flag)
      .map(|m| {
        // m.current_transaction_flag = false;
        m.message.clone()
      })
      .collect::<Vec<ChatCompletionRequestMessage>>();
    // debug_request_validation(&request);
    // let request = self.request_message_buffer.clone().unwrap();
    // let token_count = self.request_buffer_token_count;
    tx.send(Action::UpdateStatus(Some("Assembling request...".to_string())))
      .unwrap();
    tokio::spawn(async move {
      let mut embeddings_and_messages = match (input, rag) {
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
      };

      for message in new_messages {
        embeddings_and_messages.push(message);
      }

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
                tx.send(Action::AddMessage(ChatMessage::StreamResponse(vec![
                  response,
                ])))
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
            tx.send(Action::AddMessage(ChatMessage::Response(response)))
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
