use ansi_to_tui::IntoText;
use async_openai::types::{
  ChatCompletionRequestMessage, CreateChatCompletionRequest, CreateEmbeddingRequestArgs, CreateEmbeddingResponse,
  FunctionCall, Role,
};

use crossterm::event::{KeyCode, KeyEvent};
use futures::StreamExt;
use ratatui::layout::Rect;
use ratatui::{prelude::*, widgets::block::*, widgets::*};
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::result::Result;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, io};
use tokio::sync::mpsc::UnboundedSender;

use async_openai::{config::OpenAIConfig, Client};

use backoff::exponential::ExponentialBackoffBuilder;

use super::{Component, Frame};
use crate::app::{consts::*, errors::*, tools::chunkifier::*, types::*};
use crate::trace_dbg;
use crate::tui::Event;
use crate::{action::Action, config::Config};

use crate::app::gpt_interface::{
  cargo_check, create_chat_completion_function_args, create_file, define_commands, file_search,
  get_accessible_file_paths, modify_file, read_file_lines,
};
use crate::app::tools::utils::ensure_directory_exists;
use crate::components::home::Mode;

#[derive(Serialize, Deserialize, Debug, Clone)]

pub struct SessionConfig {
  pub prompt: String,
  pub session_id: String,
  pub list_file_paths: Vec<PathBuf>,
  pub model: Model,
  pub name: String,
  pub include_functions: bool,
  pub stream_response: bool,
  pub function_result_max_tokens: usize,
  pub response_max_tokens: usize,
  #[serde(skip)]
  pub openai_config: OpenAIConfig,
}

impl Default for SessionConfig {
  fn default() -> Self {
    SessionConfig {
      prompt: String::new(),
      session_id: Self::generate_session_id(),
      openai_config: OpenAIConfig::default(),
      list_file_paths: vec![],
      model: GPT3_TURBO.clone(),
      name: "Sazid Test".to_string(),
      function_result_max_tokens: 1024,
      response_max_tokens: 1024,
      include_functions: true,
      stream_response: true,
    }
  }
}
impl SessionConfig {
  pub fn with_local_api(mut self) -> Self {
    log::info!("Using local API");
    self.openai_config = OpenAIConfig::new().with_api_base("http://localhost:1234/v1".to_string());
    self
  }

  pub fn with_openai_api_key<S: Into<String>>(mut self, api_key: S) -> Self {
    log::info!("Using default OpenAI remote API");
    self.openai_config = OpenAIConfig::new().with_api_key(api_key).with_org_id("org-WagBLu0vLgiuEL12dylmcPFj");
    self
  }

  pub fn prompt_message(&self) -> ChatCompletionRequestMessage {
    ChatCompletionRequestMessage {
      content: Some(self.prompt.clone()),
      name: None,
      function_call: None,
      role: Role::User,
    }
  }

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
  pub data: SessionData,
  pub config: SessionConfig,
  #[serde(skip)]
  pub action_tx: Option<UnboundedSender<Action>>,
  #[serde(skip)]
  pub mode: Mode,
  #[serde(skip)]
  pub last_events: Vec<KeyEvent>,
  #[serde(skip)]
  pub vertical_scroll_state: ScrollbarState,
  #[serde(skip)]
  pub horizontal_scroll_state: ScrollbarState,
  #[serde(skip)]
  pub vertical_scroll: usize,
  #[serde(skip)]
  pub horizontal_scroll: usize,
  #[serde(skip)]
  pub render: bool,
  #[serde(skip)]
  pub fn_name: Option<String>,
  #[serde(skip)]
  pub fn_args: Option<String>,
}

impl Component for Session {
  fn init(&mut self, _area: Rect) -> Result<(), SazidError> {
    //let model_preference: Vec<Model> = vec![GPT4.clone(), GPT3_TURBO.clone(), WIZARDLM.clone()];
    //Session::select_model(model_preference, create_openai_client(self.config.openai_config.clone()));
    trace_dbg!("init session");
    self.config.prompt =
        "act as a programming architecture and implementation expert, that specializes in the Rust.
        Use the functions available to assist with the user inquiry.
        Do not try and execute arbitrary python code.
        Do not try to infer a path to a file, if you have not been provided a path with the root ./, use the file_search function to verify the file path before you execute a function call.".to_string();
    self.data.add_message(ChatMessage::PromptMessage(self.config.prompt_message()));
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
      Action::SubmitInput(s) => {
        self.request_response(s, tx);
      },
      Action::RequestChatCompletion() => self.request_chat_completion(),
      Action::ProcessResponse(response) => {
        self.response_handler(tx, response);
      },
      Action::CallFunction(fn_name, fn_args) => {
        self.handle_chat_response_function_call(tx, fn_name, fn_args);
      },
      Action::SelectModel(model) => self.config.model = model,
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

  fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>, SazidError> {
    self.last_events.push(key);
    match self.mode {
      Mode::Normal => match key.code {
        KeyCode::Char('j') => {
          self.vertical_scroll = self.vertical_scroll.saturating_add(1);
          self.vertical_scroll_state = self.vertical_scroll_state.position(self.vertical_scroll);
          //trace_dbg!("previous scroll {}", self.vertical_scroll);
          //self.vertical_scroll_state.prev();
          Ok(Some(Action::Update))
        },
        KeyCode::Char('k') => {
          self.vertical_scroll = self.vertical_scroll.saturating_sub(1);
          self.vertical_scroll_state = self.vertical_scroll_state.position(self.vertical_scroll);
          //trace_dbg!("next scroll {}", self.vertical_scroll);
          //self.vertical_scroll_state.next();
          Ok(Some(Action::Update))
        },
        _ => Ok(None),
      },
      _ => Ok(None),
    }
  }

  fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<(), SazidError> {
    //trace_dbg!("calling draw from session");
    let rects = Layout::default()
      .direction(Direction::Vertical)
      .constraints([Constraint::Percentage(100), Constraint::Min(4)].as_ref())
      .split(area);
    let inner_a = Layout::default()
      .direction(Direction::Vertical)
      .constraints(vec![Constraint::Length(1), Constraint::Min(10), Constraint::Length(1)])
      .split(rects[0]);
    let inner = Layout::default()
      .direction(Direction::Horizontal)
      .constraints(vec![Constraint::Length(2), Constraint::Min(10), Constraint::Length(1)])
      .split(inner_a[1]);

    let _title = "Chat";

    let block = Block::default().borders(Borders::NONE).gray();
    // .title(Title::from("left").alignment(Alignment::Left));
    //.title(Title::from("right").alignment(Alignment::Right));
    let paragraph = Paragraph::new(self.data.stylized_text.join("\n").into_text().unwrap_or_default())
      .block(block)
      .wrap(Wrap { trim: true })
      .scroll((self.vertical_scroll as u16, 0));
    f.render_widget(paragraph, inner[1]);

    f.render_stateful_widget(
      Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓")),
      inner[1],
      &mut self.vertical_scroll_state,
    );
    //self.render = false;
    Ok(())
  }
}

impl Session {
  pub fn new() -> Session {
    Self::default()
  }

  pub fn add_chunked_chat_completion_request_messages(
    &mut self,
    content: &str,
    _name: &str,
    role: Role,
    model: &Model,
    function_call: Option<FunctionCall>,
  ) -> Result<(), ChunkifierError> {
    match parse_input(content, CHUNK_TOKEN_LIMIT as usize, model.token_limit as usize) {
      Ok(chunks) => {
        chunks.iter().for_each(|chunk| {
          self.data.add_message(ChatMessage::ChatCompletionRequestMessage(ChatCompletionRequestMessage {
            role: role.clone(),
            //name: Some(name.to_string()),
            content: Some(chunk.clone()),
            function_call: function_call.clone(),
            ..Default::default()
          }))
        });
        Ok(())
      },
      Err(e) => Err(e),
    }
  }
  pub fn construct_request(&self) -> CreateChatCompletionRequest {
    let functions = match self.config.include_functions {
      true => Some(create_chat_completion_function_args(define_commands())),
      false => None,
    };
    CreateChatCompletionRequest {
      model: self.config.model.name.clone(),
      messages: self.data.messages.iter().map(|m| m.into()).collect::<Vec<ChatCompletionRequestMessage>>(),
      functions,
      stream: Some(self.config.stream_response),
      max_tokens: Some(self.config.response_max_tokens as u16),
      ..Default::default()
    }
  }

  pub fn request_response(&mut self, input: String, tx: UnboundedSender<Action>) {
    let config = self.config.clone();
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

  pub fn handle_chat_response_function_call(&mut self, tx: UnboundedSender<Action>, fn_name: String, fn_args: String) {
    trace_dbg!("handle function name:");
    trace_dbg!(&fn_name);
    trace_dbg!(&fn_args);

    match self.execute_function_call(fn_name, fn_args) {
      Ok(Some(output)) => {
        self.data.add_message(ChatMessage::ChatCompletionRequestMessage(ChatCompletionRequestMessage {
          name: Some("Sazid".to_string()),
          role: Role::Function,
          content: Some(output),
          ..Default::default()
        }));
      },
      Ok(None) => {},
      Err(e) => {
        self.data.add_message(ChatMessage::ChatCompletionRequestMessage(ChatCompletionRequestMessage {
          name: Some("Sazid".to_string()),
          role: Role::Function,
          content: Some(format!("Error: {:?}", e)),
          ..Default::default()
        }));
      },
    }
    tx.send(Action::RequestChatCompletion()).unwrap();
  }

  pub fn request_chat_completion(&mut self) {
    let tx = self.action_tx.clone().unwrap();
    let request = self.construct_request();
    let stream_response = self.config.stream_response;

    let openai_config = self.config.openai_config.clone();
    tx.send(Action::EnterProcessing).unwrap();
    let client = create_openai_client(openai_config);
    trace_dbg!("Full Request:\n{:?}", &request);
    trace_dbg!("Sending Request:\n{:?}", &request.messages.last().unwrap().content);
    // let debug = format!("request: {:#?}", request).replace("\\n", "\n");
    // for line in debug.lines() {
    //   trace_dbg!(line);
    // }
    tokio::spawn(async move {
      match stream_response {
        true => {
          // let mut stream: Pin<Box<dyn StreamExt<Item = Result<CreateChatCompletionStreamResponse, OpenAIError>> + Send>> =
          let mut stream = client.chat().create_stream(request.clone()).await.unwrap();
          while let Some(response_result) = stream.next().await {
            match response_result {
              Ok(response) => {
                trace_dbg!("Response: {:?}", response.clone());
                tx.send(Action::ProcessResponse(ChatResponse::StreamResponse(response))).unwrap();
              },
              Err(e) => {
                trace_dbg!("Error: {:?} -- check https://status.openai.com/", e);
                trace_dbg!("Request: \n{:?}", request.clone());
                tx.send(Action::Error(format!("Error: {:?} -- check https://status.openai.com/", e))).unwrap();
              },
            }
          }
        },
        false => match client.chat().create(request).await {
          Ok(response) => {
            tx.send(Action::ProcessResponse(ChatResponse::Response(response))).unwrap();
          },
          Err(e) => {
            trace_dbg!("Error: {}", e);
            tx.send(Action::Error(format!("Error: {:#?} -- check https://status.openai.com/", e))).unwrap();
          },
        },
      };
      tx.send(Action::ExitProcessing).unwrap();
    });
  }

  pub fn response_handler(&mut self, tx: UnboundedSender<Action>, response: ChatResponse) {
    for choice in <Vec<ChatMessage>>::from(response) {
      self.data.add_message(choice);
    }
    tx.send(Action::Update).unwrap();
  }
  pub fn execute_function_call(&self, fn_name: String, fn_args: String) -> Result<Option<String>, FunctionCallError> {
    let function_args: Result<HashMap<String, Value>, serde_json::Error> = serde_json::from_str(fn_args.as_str());
    match function_args {
      Ok(function_args) => {
        if let Some(v) = function_args.get("path") {
          if let Some(pathstr) = v.as_str() {
            let accesible_paths = get_accessible_file_paths(self.config.list_file_paths.clone());
            if !accesible_paths.contains_key(Path::new(pathstr).to_str().unwrap()) {
              return Err(FunctionCallError::new(
                format!("File path is not accessible: {:?}. Suggest using file_search command", pathstr).as_str(),
              ));
            } else {
              trace_dbg!("path: {:?} exists", pathstr);
            }
          }
        }
        let start_line: Option<usize> = function_args.get("start_line").and_then(|s| s.as_u64().map(|u| u as usize));
        let end_line: Option<usize> = function_args.get("end_line").and_then(|s| s.as_u64().map(|u| u as usize));
        let search_term: Option<&str> = function_args.get("search_term").and_then(|s| s.as_str());

        match fn_name.as_str() {
          "create_file" => create_file(
            function_args["path"].as_str().unwrap_or_default(),
            function_args["text"].as_str().unwrap_or_default(),
          ),
          "file_search" => {
            file_search(self.config.function_result_max_tokens, self.config.list_file_paths.clone(), search_term)
          },
          "read_lines" => read_file_lines(
            function_args["path"].as_str().unwrap_or_default(),
            start_line,
            end_line,
            self.config.function_result_max_tokens,
            self.config.list_file_paths.clone(),
          ),
          "modify_file" => modify_file(
            function_args["path"].as_str().unwrap(),
            start_line.unwrap_or(0),
            end_line,
            function_args["insert_text"].as_str(),
          ),
          "cargo_check" => cargo_check(),
          _ => Ok(None),
        }
      },
      Err(e) => Err(FunctionCallError::new(
        format!("Failed to parse function arguments:\nfunction:{:?}\nargs:{:?}\nerror:{:?}", fn_name, fn_args, e)
          .as_str(),
      )),
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
  // let api_key: String = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");
  // let openai_config = OpenAIConfig::new().with_api_key(api_key);
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

#[cfg(test)]
mod tests {}
