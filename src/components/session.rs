use async_openai::types::{
  ChatCompletionFunctions, ChatCompletionRequestMessage, ChatCompletionStreamResponseDelta,
  CreateChatCompletionRequest, CreateEmbeddingRequestArgs, CreateEmbeddingResponse, FunctionCall, FunctionCallStream,
  Role,
};
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use futures::StreamExt;
use ratatui::layout::Rect;
use ratatui::{prelude::*, widgets::block::*, widgets::*};
use serde_derive::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
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

use crate::app::gpt_interface::{create_chat_completion_function_args, define_commands};
use crate::app::tools::utils::ensure_directory_exists;
use crate::components::home::Mode;

#[derive(Serialize, Deserialize, Debug, Clone)]

pub struct SessionConfig {
  pub session_id: String,
  pub model: Model,
  pub include_functions: bool,
  pub stream_response: bool,
  #[serde(skip)]
  pub openai_config: OpenAIConfig,
}

impl Default for SessionConfig {
  fn default() -> Self {
    SessionConfig {
      session_id: Self::generate_session_id(),
      openai_config: OpenAIConfig::default(),
      model: GPT4.clone(),
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
    self.openai_config = OpenAIConfig::new().with_api_key(api_key);
    self
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
  pub transactions: Vec<Transaction>,
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
}

impl Component for Session {
  fn init(&mut self, _area: Rect) -> Result<()> {
    //let model_preference: Vec<Model> = vec![GPT4.clone(), GPT3_TURBO.clone(), WIZARDLM.clone()];
    //Session::select_model(model_preference, create_openai_client(self.config.openai_config.clone()));
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
    let tx = self.action_tx.clone().unwrap();
    match action {
      Action::SubmitInput(s) => {
        self.request_response(s, tx);
      },
      Action::ProcessResponse(boxed_id_response) => {
        let (transaction_id, response) = *boxed_id_response;
        self.process_response_handler(tx, transaction_id, response);
      },
      Action::SelectModel(model) => self.config.model = model,
      _ => (),
    }
    //self.action_tx.clone().unwrap().send(Action::Render).unwrap();
    Ok(None)
  }

  fn handle_events(&mut self, event: Option<Event>) -> Result<Option<Action>> {
    let r = match event {
      Some(Event::Key(key_event)) => self.handle_key_events(key_event)?,
      Some(Event::Mouse(mouse_event)) => self.handle_mouse_events(mouse_event)?,
      _ => None,
    };
    Ok(r)
  }

  fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
    self.last_events.push(key);
    match self.mode {
      Mode::Normal => match key.code {
        KeyCode::Char('j') => {
          self.vertical_scroll = self.vertical_scroll.saturating_add(1);
          self.vertical_scroll_state = self.vertical_scroll_state.position(self.vertical_scroll);
          trace_dbg!("previous scroll {}", self.vertical_scroll);
          //self.vertical_scroll_state.prev();
          Ok(Some(Action::Update))
        },
        KeyCode::Char('k') => {
          self.vertical_scroll = self.vertical_scroll.saturating_sub(1);
          self.vertical_scroll_state = self.vertical_scroll_state.position(self.vertical_scroll);
          trace_dbg!("next scroll {}", self.vertical_scroll);
          //self.vertical_scroll_state.next();
          Ok(Some(Action::Update))
        },
        _ => Ok(None),
      },
      _ => Ok(None),
    }
  }

  fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
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
    let paragraph = Paragraph::new(Text::<'_>::from(self.get_full_text()))
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

fn concatenate_texts<'a, I>(texts: I) -> Text<'a>
where
  I: Iterator<Item = Text<'a>>,
{
  let mut result = Text::raw("");
  for mut text in texts {
    result.lines.append(text.lines.as_mut());
  }
  result
}

impl Session {
  pub fn new() -> Session {
    Self::default()
  }

  fn get_previous_request_messages(&self) -> Vec<ChatCompletionRequestMessage> {
    self.transactions.iter().map(|transaction| transaction.request.messages.clone()).flatten().collect()
  }
  pub fn get_full_text(&mut self) -> String {
    self.transactions.iter().map(|transaction| <String>::from(transaction)).collect::<Vec<String>>().join("\n")
  }

  pub fn request_response(&mut self, input: String, tx: UnboundedSender<Action>) {
    //let tx = self.action_tx.clone().unwrap();
    let previous_requests = self.get_previous_request_messages();
    let request_messages =
      construct_chat_completion_request_message(&input, Role::User, &self.config.model, Some(previous_requests), None)
        .unwrap();
    let functions = match self.config.include_functions {
      true => Some(create_chat_completion_function_args(define_commands())),
      false => None,
    };
    let request = construct_request(request_messages, &self.config, functions);
    let stream_response = self.config.stream_response;
    let openai_config = self.config.openai_config.clone();
    format!("Request: {:#?}", request.clone());

    tx.send(Action::EnterProcessing).unwrap();
    let client = create_openai_client(openai_config);
    let txn = Transaction::new(request);
    self.transactions.push(txn.clone());
    let request = txn.clone().request;
    let id = txn.clone().id;
    tokio::spawn(async move {
      match stream_response {
        true => {
          // let mut stream: Pin<Box<dyn StreamExt<Item = Result<CreateChatCompletionStreamResponse, OpenAIError>> + Send>> =
          let mut stream = client.chat().create_stream(request).await.unwrap();
          while let Some(response_result) = stream.next().await {
            match response_result {
              Ok(response) => {
                trace_dbg!("Response: {:#?}", response);
                tx.send(Action::ProcessResponse(Box::new((id.clone(), ChatResponse::StreamResponse(response)))))
                  .unwrap();
              },
              Err(e) => {
                trace_dbg!("Error: {:#?} -- check https://status.openai.com/", e);
                tx.send(Action::Error(format!("Error: {:#?} -- check https://status.openai.com/", e))).unwrap();
              },
            }
          }
        },
        false => match client.chat().create(request).await {
          Ok(response) => {
            tx.send(Action::ProcessResponse(Box::new((id, ChatResponse::Response(response))))).unwrap();
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

  pub fn process_response_handler(
    &mut self,
    tx: UnboundedSender<Action>,
    transaction_id: String,
    response: ChatResponse,
  ) {
    trace_dbg!("response handler");
    self.transactions.iter_mut().find(|txn| txn.id == transaction_id).unwrap().responses.push(response.clone());
    tx.send(Action::Update).unwrap();
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

pub fn construct_chat_completion_request_message(
  content: &str,
  role: Role,
  model: &Model,
  previous_requests: Option<Vec<ChatCompletionRequestMessage>>,
  function_call: Option<FunctionCall>,
) -> Result<Vec<ChatCompletionRequestMessage>, GPTConnectorError> {
  let chunks = parse_input(content, CHUNK_TOKEN_LIMIT as usize, model.token_limit as usize).unwrap();

  let messages: Vec<ChatCompletionRequestMessage> = chunks
    .iter()
    .map(|chunk| ChatCompletionRequestMessage {
      role: role.clone(),
      content: Some(chunk.clone()),
      function_call: function_call.clone(),
      ..Default::default()
    })
    .collect();
  match previous_requests {
    Some(mut previous_requests) => {
      previous_requests.append(&mut messages.clone());
      Ok(previous_requests)
    },
    None => Ok(messages),
  }
}

pub fn construct_request(
  messages: Vec<ChatCompletionRequestMessage>,
  config: &SessionConfig,                          // model: Model,
  functions: Option<Vec<ChatCompletionFunctions>>, // include_functions: bool,
) -> CreateChatCompletionRequest {
  CreateChatCompletionRequest {
    model: config.model.name.clone(),
    messages,
    functions,
    stream: Some(config.stream_response),
    max_tokens: Some(1024),
    ..Default::default()
  }
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
