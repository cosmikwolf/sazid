use ansi_to_tui::IntoText;
use async_openai::types::{
  ChatCompletionRequestMessage, CreateChatCompletionRequest, CreateEmbeddingRequestArgs, CreateEmbeddingResponse,
  FunctionCall, Role,
};

use color_eyre::owo_colors::OwoColorize;
use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use futures::StreamExt;
use ratatui::layout::Rect;
use ratatui::{prelude::*, widgets::block::*, widgets::*};
use serde_derive::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::result::Result;
use std::{fs, io};
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::TextArea;

use async_openai::{config::OpenAIConfig, Client};

use backoff::exponential::ExponentialBackoffBuilder;

use super::{Component, Frame};
use crate::app::functions::{all_functions, handle_chat_response_function_call};
use crate::app::messages::{ChatMessage, ChatResponse};
use crate::app::session_config::SessionConfig;
use crate::app::session_data::SessionData;
use crate::app::session_view::SessionView;
use crate::app::{consts::*, errors::*, tools::chunkifier::*, types::*};
use crate::trace_dbg;
use crate::tui::Event;
use crate::{action::Action, config::Config};

use crate::app::gpt_interface::create_chat_completion_function_args;
use crate::app::tools::utils::ensure_directory_exists;
use crate::components::home::Mode;

#[derive(Serialize, Deserialize, Debug)]
pub struct Session<'a> {
  pub data: SessionData,
  pub config: SessionConfig,
  #[serde(skip)]
  pub view: SessionView,
  #[serde(skip)]
  pub action_tx: Option<UnboundedSender<Action>>,
  #[serde(skip)]
  pub mode: Mode,
  #[serde(skip)]
  pub last_events: Vec<KeyEvent>,
  #[serde(skip)]
  pub text_area: TextArea<'a>,
  #[serde(skip)]
  pub vertical_scroll_state: ScrollbarState,
  #[serde(skip)]
  pub horizontal_scroll_state: ScrollbarState,
  #[serde(skip)]
  pub vertical_scroll: usize,
  #[serde(skip)]
  pub horizontal_scroll: usize,
  #[serde(skip)]
  pub scroll_max: usize,
  #[serde(skip)]
  pub vertical_content_height: usize,
  #[serde(skip)]
  pub vertical_viewport_height: usize,
  #[serde(skip)]
  pub scroll_sticky_end: bool,
  #[serde(skip)]
  pub render: bool,
  #[serde(skip)]
  pub fn_name: Option<String>,
  #[serde(skip)]
  pub fn_args: Option<String>,
  #[serde(skip)]
  pub request_buffer_with_token_count: Vec<(Option<ChatCompletionRequestMessage>, usize)>,
  #[serde(skip)]
  pub request_message_buffer: Option<CreateChatCompletionRequest>,
  #[serde(skip)]
  pub request_buffer_token_count: usize,
  #[serde(skip)]
  pub input_vsize: u16,
}

impl<'a> Default for Session<'a> {
  fn default() -> Self {
    Session {
      data: SessionData::default(),
      config: SessionConfig::default(),
      action_tx: None,
      mode: Mode::Normal,
      last_events: vec![],
      text_area: TextArea::default(),
      vertical_scroll_state: ScrollbarState::default(),
      view: SessionView::default(),
      horizontal_scroll_state: ScrollbarState::default(),
      vertical_scroll: 0,
      scroll_max: 0,
      horizontal_scroll: 0,
      vertical_content_height: 0,
      vertical_viewport_height: 0,
      scroll_sticky_end: true,
      render: false,
      fn_name: None,
      fn_args: None,
      request_buffer_with_token_count: Vec::new(),
      request_message_buffer: None,
      request_buffer_token_count: 0,
      input_vsize: 1,
    }
  }
}

impl Component for Session<'static> {
  fn init(&mut self, area: Rect) -> Result<(), SazidError> {
    let tx = self.action_tx.clone().unwrap();
    //let model_preference: Vec<Model> = vec![GPT4.clone(), GPT3_TURBO.clone(), WIZARDLM.clone()];
    //Session::select_model(model_preference, create_openai_client(self.config.openai_config.clone()));
    trace_dbg!("init session");
    //     self.config.prompt =
    // "act as a programming architecture and implementation expert, that specializes in the Rust.
    // Use the functions available to assist with the user inquiry.
    // Provide your response as markdown formatted text.
    // Make sure to properly entabulate any code blocks
    // Do not try and execute arbitrary python code.
    // Do not try to infer a path to a file, if you have not been provided a path with the root ./, use the file_search function to verify the file path before you execute a function call.".to_string();
    self.config.prompt = "act as a very terse assistant".into();
    self.view.set_window_width(area.width as usize, &mut self.data.messages);
    tx.send(Action::AddMessage(ChatMessage::PromptMessage(self.config.prompt_message()))).unwrap();
    self.view.post_process_new_messages(&mut self.data);
    self.config.available_functions = all_functions();
    Ok(())
  }
  fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<(), SazidError> {
    trace_dbg!("register_session_action_handler");
    self.action_tx = Some(tx);
    Ok(())
  }
  fn register_config_handler(&mut self, config: Config) -> Result<(), SazidError> {
    self.config = config.session_config;
    Ok(())
  }
  fn update(&mut self, action: Action) -> Result<Option<Action>, SazidError> {
    let tx = self.action_tx.clone().unwrap();
    match action {
      Action::AddMessage(chat_message) => {
        //trace_dbg!(level: tracing::Level::INFO, "adding message to session");
        self.data.add_message(chat_message);
        self.view.post_process_new_messages(&mut self.data);
        for function_call in self.data.get_functions_that_need_calling().drain(..) {
          let debug_text = format!("calling function: {:?}", function_call);
          trace_dbg!(level: tracing::Level::INFO, debug_text);
          handle_chat_response_function_call(
            tx.clone(),
            function_call.name,
            function_call.arguments,
            self.config.clone(),
          );
        }
        self.add_new_messages_to_request_buffer();
      },
      Action::SubmitInput(s) => {
        self.scroll_sticky_end = true;
        self.submit_chat_completion_request(s, tx);
      },
      Action::RequestChatCompletion() => {
        trace_dbg!(level: tracing::Level::INFO, "requesting chat completion");
        self.request_chat_completion(tx.clone())
      },
      Action::SelectModel(model) => self.config.model = model,
      Action::SetInputVsize(vsize) => {
        self.input_vsize = vsize;
      },
      _ => (),
    }
    //self.action_tx.clone().unwrap().send(Action::Render).unwrap();
    Ok(None)
  }

  fn handle_events(&mut self, event: Option<Event>) -> Result<Option<Action>, SazidError> {
    let r = match event {
      Some(Event::Key(key_event)) => self.handle_key_events(key_event)?,
      Some(Event::Mouse(mouse_event)) => self.handle_mouse_events(mouse_event)?,
      _ => None,
    };
    Ok(r)
  }

  fn handle_mouse_events(&mut self, mouse_event: MouseEvent) -> Result<Option<Action>, SazidError> {
    match mouse_event {
      MouseEvent { kind: MouseEventKind::ScrollUp, .. } => self.scroll_up(),
      MouseEvent { kind: MouseEventKind::ScrollDown, .. } => self.scroll_down(),
      _ => Ok(None),
    }
  }

  fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>, SazidError> {
    self.last_events.push(key);
    match self.mode {
      Mode::Normal => match key.code {
        KeyCode::Char('j') => self.scroll_down(),
        KeyCode::Char('k') => self.scroll_up(),
        _ => Ok(None),
      },
      _ => Ok(None),
    }
  }

  fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<(), SazidError> {
    let rects = Layout::default()
      .direction(Direction::Vertical)
      .constraints([Constraint::Percentage(100), Constraint::Min(self.input_vsize)].as_ref())
      .split(area);
    let inner_a = Layout::default()
      .direction(Direction::Vertical)
      .constraints(vec![Constraint::Length(1), Constraint::Min(10), Constraint::Length(0)])
      .split(rects[0]);
    let inner = Layout::default()
      .direction(Direction::Horizontal)
      .constraints(vec![Constraint::Length(3), Constraint::Min(10), Constraint::Length(3)])
      .split(inner_a[1]);

    let block = Block::default().borders(Borders::NONE).gray();

    self.vertical_viewport_height = inner[1].height as usize;
    self.vertical_content_height = self.view.rendered_text.len_lines();
    self.vertical_scroll_state = self.vertical_scroll_state.content_length(self.vertical_content_height);
    self.view.set_window_width(inner[1].width as usize, &mut self.data.messages);
    self.scroll_max = self.view.rendered_text.len_lines().saturating_sub(self.vertical_viewport_height);
    // + self.vertical_viewport_height.min(3);
    self.vertical_scroll_state = self.vertical_scroll_state.viewport_content_length(self.vertical_content_height);

    if self.scroll_sticky_end {
      //self.vertical_scroll_state.last();
      self.vertical_scroll = self.scroll_max;
      self.vertical_scroll_state = self.vertical_scroll_state.position(self.vertical_scroll);
    }

    let text =
      self.view.get_stylized_rendered_slice(self.vertical_scroll, self.vertical_viewport_height, self.vertical_scroll);
    let paragraph = Paragraph::new(text.into_text().unwrap())
      .block(block)
      .wrap(Wrap { trim: false })
      .scroll((self.vertical_scroll as u16, 0));
    let scrollbar = Scrollbar::default()
      .orientation(ScrollbarOrientation::VerticalRight)
      .thumb_symbol("󱁨")
      .begin_symbol(Some("󰶼"))
      .end_symbol(Some("󰶹"));
    f.render_widget(paragraph, inner[1]);
    f.render_stateful_widget(scrollbar, inner[2], &mut self.vertical_scroll_state);
    //self.render = false;
    Ok(())
  }
}

fn _create_empty_lines(n: usize) -> String {
  let mut s = String::with_capacity(n + 1);
  for _ in 0..n {
    s.push('\n');
  }
  s
}

impl Session<'static> {
  pub fn new() -> Session<'static> {
    Self::default()
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
    self.vertical_scroll = self.vertical_scroll.saturating_add(1).min(self.scroll_max);
    self.vertical_scroll_state = self.vertical_scroll_state.position(self.vertical_scroll);
    if self.vertical_scroll_state == self.vertical_scroll_state.position(self.scroll_max) {
      if !self.scroll_sticky_end {
        let mut debug_string = String::new();
        for (idx, line) in self.view.rendered_text.lines().enumerate() {
          debug_string.push_str(format!("{:02}\t", idx).as_str());
          debug_string.push_str(line.to_string().as_str());
        }
      }
      self.scroll_sticky_end = true;
    }
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
    _name: &str,
    role: Role,
    model: &Model,
    function_call: Option<FunctionCall>,
  ) -> Result<(), SazidError> {
    match parse_input(content, CHUNK_TOKEN_LIMIT as usize, model.token_limit as usize) {
      Ok(chunks) => {
        chunks.iter().for_each(|chunk| {
          self.data.add_message(ChatMessage::ChatCompletionRequestMessage(ChatCompletionRequestMessage {
            role,
            name: None,
            content: Some(chunk.clone()),
            function_call: function_call.clone(),
          }));
        });
        Ok(())
      },
      Err(e) => Err(SazidError::ChunkifierError(e)),
    }
  }

  pub fn add_new_messages_to_request_buffer(&mut self) {
    // add new request messages to the request buffer
    let new_requests: Vec<(Option<ChatCompletionRequestMessage>, usize)> = self
      .data
      .messages
      .iter()
      .filter(|m| match m.message {
        ChatMessage::ChatCompletionRequestMessage(_) => true,
        ChatMessage::PromptMessage(_) => true,
        ChatMessage::ChatCompletionResponseMessage(_) => true,
        ChatMessage::FunctionResult(_) => true,
        ChatMessage::SazidSystemMessage(_) => false,
      })
      .skip(self.request_buffer_with_token_count.len())
      .filter(|m| m.finished)
      .map(|m| {
        let debug = format!("message: {:#?}", m.message).bright_red().to_string();
        trace_dbg!(debug);
        (<Option<ChatCompletionRequestMessage>>::from(&m.message), m.get_token_count())
      })
      .collect();
    self.request_buffer_with_token_count.extend(new_requests);
    trace_dbg!("request_buffer_with_token_count: {:#?}", self.request_buffer_with_token_count);
  }

  pub fn construct_request(&mut self) {
    let functions = match self.config.available_functions.is_empty() {
      true => None,
      false => {
        Some(create_chat_completion_function_args(self.config.available_functions.iter().map(|f| f.into()).collect()))
      },
    };

    let mut token_count = 0;

    let request = CreateChatCompletionRequest {
      model: self.config.model.name.clone(),
      functions,
      messages: self
        .request_buffer_with_token_count
        .clone()
        .into_iter()
        .flat_map(|(o, tc)| {
          token_count += tc;
          o
        })
        .collect(),
      stream: Some(self.config.stream_response),
      max_tokens: Some(self.config.response_max_tokens as u16),
      // todo: put the user information in here
      user: Some("testing testing".to_string()),
      ..Default::default()
    };
    self.request_message_buffer = Some(request)
  }

  pub fn submit_chat_completion_request(&mut self, input: String, tx: UnboundedSender<Action>) {
    let config = self.config.clone();
    tx.send(Action::UpdateStatus(Some("submitting input".to_string()))).unwrap();
    match self.add_chunked_chat_completion_request_messages(
      &input,
      config.name.as_str(),
      Role::User,
      &config.model,
      None,
    ) {
      Ok(_) => {
        tx.send(Action::RequestChatCompletion()).unwrap();
      },
      Err(e) => {
        tx.send(Action::Error(format!("Error: {:?}", e))).unwrap();
      },
    }
  }

  // fn handle_chat_response_function_call(
  //   tx: UnboundedSender<Action>,
  //   fn_name: String,
  //   fn_args: String,
  //   session_config: SessionConfig,
  // ) {
  //   tokio::spawn(async move {
  //     match {
  //       let fn_name = fn_name.clone();
  //       let fn_args = fn_args;
  //       async move {
  //         let function_args: Result<HashMap<String, Value>, serde_json::Error> = serde_json::from_str(fn_args.as_str());
  //         match function_args {
  //           Ok(function_args) => {
  //             if let Some(v) = function_args.get("path") {
  //               if let Some(pathstr) = v.as_str() {
  //                 let accesible_paths = get_accessible_file_paths(session_config.list_file_paths.clone());
  //                 if !accesible_paths.contains_key(Path::new(pathstr).to_str().unwrap()) {
  //                   return Err(FunctionCallError::new(
  //                     format!("File path is not accessible: {:?}. Suggest using file_search command", pathstr).as_str(),
  //                   ));
  //                 } else {
  //                   trace_dbg!("path: {:?} exists", pathstr);
  //                 }
  //               }
  //             }
  //             let start_line: Option<usize> =
  //               function_args.get("start_line").and_then(|s| s.as_u64().map(|u| u as usize));
  //             let end_line: Option<usize> = function_args.get("end_line").and_then(|s| s.as_u64().map(|u| u as usize));
  //             let search_term: Option<&str> = function_args.get("search_term").and_then(|s| s.as_str());
  //
  //             match fn_name.as_str() {
  //               "create_file" => create_file(
  //                 function_args["path"].as_str().unwrap_or_default(),
  //                 function_args["text"].as_str().unwrap_or_default(),
  //               ),
  //               "grep" => crate::app::gpt_interface::grep(
  //                 function_args["pattern"].as_str().unwrap_or_default(),
  //                 function_args["paths"].as_str().unwrap_or_default(),
  //               ),
  //               "file_search" => file_search(
  //                 session_config.function_result_max_tokens,
  //                 session_config.list_file_paths.clone(),
  //                 search_term,
  //               ),
  //               "read_lines" => read_file_lines(
  //                 function_args["path"].as_str().unwrap_or_default(),
  //                 start_line,
  //                 end_line,
  //                 session_config.function_result_max_tokens,
  //                 session_config.list_file_paths.clone(),
  //               ),
  //               "modify_file" => modify_file(
  //                 function_args["path"].as_str().unwrap(),
  //                 start_line.unwrap_or(0),
  //                 end_line,
  //                 function_args["insert_text"].as_str(),
  //               ),
  //               "cargo_check" => cargo_check(),
  //               _ => Ok(None),
  //             }
  //           },
  //           Err(e) => Err(FunctionCallError::new(
  //             format!("Failed to parse function arguments:\nfunction:{:?}\nargs:{:?}\nerror:{:?}", fn_name, fn_args, e)
  //               .as_str(),
  //           )),
  //         }
  //       }
  //     }
  //     .await
  //     {
  //       Ok(Some(output)) => {
  //         //self.data.add_message(ChatMessage::FunctionResult(FunctionResult { name: fn_name, response: output }));
  //         tx.send(Action::AddMessage(ChatMessage::FunctionResult(FunctionResult { name: fn_name, response: output })))
  //           .unwrap();
  //       },
  //       Ok(None) => {},
  //       Err(e) => {
  //         // self.data.add_message(ChatMessage::FunctionResult(FunctionResult {
  //         //   name: fn_name,
  //         //   response: format!("Error: {:?}", e),
  //         // }));
  //         tx.send(Action::AddMessage(ChatMessage::FunctionResult(FunctionResult {
  //           name: fn_name,
  //           response: format!("Error: {:?}", e),
  //         })))
  //         .unwrap();
  //       },
  //     }
  //     tx.send(Action::RequestChatCompletion()).unwrap();
  //   });
  // }

  pub fn request_chat_completion(&mut self, tx: UnboundedSender<Action>) {
    tx.send(Action::UpdateStatus(Some("Configuring Client".to_string()))).unwrap();
    let stream_response = self.config.stream_response;
    let openai_config = self.config.openai_config.clone();

    if self.request_message_buffer.is_none() {
      self.construct_request()
    }
    let request = self.request_message_buffer.clone().unwrap();
    let token_count = self.request_buffer_token_count;
    tx.send(Action::UpdateStatus(Some("Assembling request...".to_string()))).unwrap();
    tokio::spawn(async move {
      tx.send(Action::UpdateStatus(Some("Establishing Client Connection".to_string()))).unwrap();
      tx.send(Action::EnterProcessing).unwrap();
      let client = create_openai_client(openai_config);
      tx.send(Action::AddMessage(ChatMessage::SazidSystemMessage(format!("Request Token Count: {}", token_count))))
        .unwrap();
      match stream_response {
        true => {
          tx.send(Action::UpdateStatus(Some("Sending Request to OpenAI API...".to_string()))).unwrap();
          let mut stream = client.chat().create_stream(request).await.unwrap();
          tx.send(Action::UpdateStatus(Some("Request submitted. Awaiting Response...".to_string()))).unwrap();
          let mut count = 0;
          while let Some(response_result) = stream.next().await {
            match response_result {
              Ok(response) => {
                count += 1;
                tx.send(Action::UpdateStatus(Some(format!("Received responses: {}", count).to_string()))).unwrap();
                Self::response_handler(tx.clone(), ChatResponse::StreamResponse(response)).await;
              },
              Err(e) => {
                trace_dbg!("Error: {:?} -- check https://status.openai.com", e);
                // let reqtext =
                //   format!("Request: \n{}", to_string_pretty(&request).unwrap_or("can't prettify result".to_string()));
                // trace_dbg!(&reqtext);
                // debug_request_validation(&request);
                // tx.send(Action::AddMessage(ChatMessage::SazidSystemMessage(reqtext))).unwrap();
                tx.send(Action::Error(format!("Error: {:?} -- check https://status.openai.com/", e))).unwrap();
              },
            }
          }
        },
        false => match client.chat().create(request).await {
          Ok(response) => {
            Self::response_handler(tx.clone(), ChatResponse::Response(response)).await;
          },
          Err(e) => {
            trace_dbg!("Error: {}", e);
            tx.send(Action::Error(format!("Error: {:#?} -- check https://status.openai.com/", e))).unwrap();
          },
        },
      };
      tx.send(Action::UpdateStatus(Some("Chat Request Complete".to_string()))).unwrap();
      tx.send(Action::ExitProcessing).unwrap();
    });
  }

  pub async fn response_handler(tx: UnboundedSender<Action>, response: ChatResponse) {
    for choice in <Vec<ChatMessage>>::from(response) {
      tx.send(Action::AddMessage(choice)).unwrap();
    }
    tx.send(Action::Update).unwrap();
  }

  pub fn load_session_by_id(session_id: String) -> Session<'static> {
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

  pub fn load_last_session() -> Session<'static> {
    let last_session_path = Path::new(SESSIONS_DIR).join("last_session.txt");
    let last_session_id = fs::read_to_string(last_session_path).unwrap();
    Self::load_session_by_id(last_session_id)
  }

  #[allow(dead_code)]
  fn save_session(&self) -> io::Result<()> {
    ensure_directory_exists(SESSIONS_DIR).unwrap();
    let session_file_path = Self::get_session_filepath(self.config.session_id.clone());
    let data = serde_json::to_string(&self)?;
    fs::write(session_file_path, data)?;
    Ok(())
  }

  pub fn save_last_session_id(&self) {
    ensure_directory_exists(SESSIONS_DIR).unwrap();
    let last_session_path = Path::new(SESSIONS_DIR).join("last_session.txt");
    fs::write(last_session_path, self.config.session_id.clone()).unwrap();
  }

  pub fn select_model(model_preference_list: Vec<Model>, client: Client<OpenAIConfig>) {
    trace_dbg!("select model");
    tokio::spawn(async move {
      // Retrieve the list of available models
      let models_response = client.models().list().await;
      match models_response {
        Ok(response) => {
          let available_models: Vec<String> = response.data.iter().map(|model| model.id.clone()).collect();
          trace_dbg!("{:?}", available_models);
          // Check if the default model is in the list
          if let Some(preferences) = model_preference_list.iter().find(|model| available_models.contains(&model.name)) {
            Ok(preferences.clone())
          } else {
            Err(SessionManagerError::Other("no preferred models available".to_string()))
          }
        },
        Err(e) => {
          trace_dbg!("Failed to fetch the list of available models. {:#?}", e);
          Err(SessionManagerError::Other("Failed to fetch the list of available models.".to_string()))
        },
      }
    });
  }
}

pub fn create_openai_client(openai_config: OpenAIConfig) -> async_openai::Client<OpenAIConfig> {
  let backoff = ExponentialBackoffBuilder::new() // Ensure backoff crate is added to Cargo.toml
    .with_max_elapsed_time(Some(std::time::Duration::from_secs(60)))
    .build();
  Client::with_config(openai_config).with_backoff(backoff)
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
