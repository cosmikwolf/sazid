use ansi_to_tui::IntoText;
use async_openai::types::{
  ChatCompletionRequestAssistantMessage, ChatCompletionRequestMessage, ChatCompletionRequestUserMessage,
  ChatCompletionRequestUserMessageContent, CreateChatCompletionRequest, CreateEmbeddingRequestArgs,
  CreateEmbeddingResponse, Role,
};
use clipboard::{ClipboardContext, ClipboardProvider};
use color_eyre::owo_colors::OwoColorize;
use crossterm::event::KeyModifiers;
use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use futures::StreamExt;
use nu_ansi_term::Color::*;
use nu_ansi_term::Style;
use ratatui::layout::Rect;
use ratatui::{prelude::*, widgets::block::*, widgets::*};
use serde_derive::{Deserialize, Serialize};
use std::default::Default;
use std::path::{Path, PathBuf};
use std::result::Result;
use std::{fs, io};
use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::TextArea;
use tui_textarea::{CursorMove, Scrolling};

use async_openai::{config::OpenAIConfig, Client};

use super::{Component, Frame};
use crate::app::functions::{all_functions, handle_tool_call};
use crate::app::helpers::list_files_ordered_by_date;
use crate::app::messages::ChatMessage;
use crate::app::request_validation::debug_request_validation;
use crate::app::session_config::SessionConfig;
use crate::app::session_data::SessionData;
use crate::app::session_view::SessionView;
use crate::app::{consts::*, errors::*, tools::chunkifier::*, types::*};
use crate::trace_dbg;
use crate::tui::Event;
use crate::{action::Action, config::Config};
use backoff::exponential::ExponentialBackoffBuilder;
use dirs_next::home_dir;

use crate::app::gpt_interface::create_chat_completion_tool_args;
use crate::app::tools::utils::ensure_directory_exists;
use crate::components::home::Mode;

#[derive(Serialize, Deserialize, Debug)]
pub struct Session<'a> {
  pub data: SessionData,
  pub config: SessionConfig,
  #[serde(skip)]
  pub view: SessionView<'a>,
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
}

impl<'a> Default for Session<'a> {
  fn default() -> Self {
    Session {
      data: SessionData::default(),
      config: SessionConfig::default(),
      action_tx: None,
      mode: Mode::Normal,
      last_events: vec![],
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
      request_buffer: Vec::new(),
      request_buffer_token_count: 0,
      input_vsize: 1,
      cursor_coords: None,
      select_start_coords: None,
      select_end_coords: None,
    }
  }
}

impl Component for Session<'static> {
  fn init(&mut self, area: Rect) -> Result<(), SazidError> {
    let tx = self.action_tx.clone().unwrap();
    //let model_preference: Vec<Model> = vec![GPT4.clone(), GPT3_TURBO.clone(), WIZARDLM.clone()];
    //Session::select_model(model_preference, create_openai_client(self.config.openai_config.clone()));
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
    self.view.set_window_width(area.width as usize, &mut self.data.messages);
    tx.send(Action::AddMessage(ChatMessage::System(self.config.prompt_message()))).unwrap();
    self.view.post_process_new_messages(&mut self.data);
    // self.text_area = TextArea::new(self.view.rendered_text.lines().map(|l| l.to_string()).collect());
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
        self.execute_tool_calls();
        self.add_new_messages_to_request_buffer();
      },
      Action::ExecuteCommand(command) => {
        tx.send(Action::CommandResult(self.execute_command(command).unwrap())).unwrap();
      },
      Action::SaveSession => {
        self.save_session().unwrap();
      },
      Action::SubmitInput(s) => {
        self.scroll_sticky_end = true;
        self.submit_chat_completion_request(s, tx);
      },
      Action::RequestChatCompletion() => {
        trace_dbg!(level: tracing::Level::INFO, "requesting chat completion");
        self.request_chat_completion(tx.clone())
      },
      Action::Resize(width, _height) => {
        self.view.set_window_width(width.into(), &mut self.data.messages);
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
      MouseEvent { kind: MouseEventKind::Drag(MouseButton::Left), column, row, modifiers } => {
        // translate mouse click coordinates to text column and row
        self.select_end_coords = Some((column as usize, row as usize));
        // self.select_coords = Some((column as usize, row as usize));
        self.cursor_coords = Some((column as usize, row as usize));
        self.view.new_data = true;
        trace_dbg!("mouse drag: column: {}, row: {}, modifiers: {:?}", column, row, modifiers);
        Ok(Some(Action::Update))
      },
      MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column, row, modifiers } => {
        // translate mouse click coordinates to text column and row
        self.select_start_coords = Some((column as usize, row as usize));
        self.cursor_coords = Some((column as usize, row as usize));
        self.view.new_data = true;
        Ok(Some(Action::Update))
      },
      _ => Ok(None),
    }
  }

  fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>, SazidError> {
    self.last_events.push(key);
    Ok(match self.mode {
      Mode::Normal => match key {
        KeyEvent { code: KeyCode::Char('d'), modifiers: KeyModifiers::CONTROL, .. } => {
          self.view.text_area.scroll(Scrolling::HalfPageDown);
          Some(Action::Update)
        },
        KeyEvent { code: KeyCode::Char('u'), modifiers: KeyModifiers::CONTROL, .. } => {
          self.view.text_area.scroll(Scrolling::HalfPageUp);
          Some(Action::Update)
        },
        KeyEvent { code: KeyCode::Char('f'), modifiers: KeyModifiers::CONTROL, .. } => {
          self.view.text_area.scroll(Scrolling::PageDown);
          Some(Action::Update)
        },
        KeyEvent { code: KeyCode::Char('b'), modifiers: KeyModifiers::CONTROL, .. } => {
          self.view.text_area.scroll(Scrolling::PageUp);
          Some(Action::Update)
        },
        KeyEvent { code: KeyCode::Char('h'), .. } => {
          self.view.text_area.move_cursor(CursorMove::Back);
          Some(Action::Update)
        },
        KeyEvent { code: KeyCode::Char('j'), .. } => {
          self.view.text_area.move_cursor(CursorMove::Down);
          trace_dbg!("cursor: {:#?}", self.view.text_area.cursor());
          Some(Action::Update)
        },
        KeyEvent { code: KeyCode::Char('k'), .. } => {
          self.view.text_area.move_cursor(CursorMove::Up);
          trace_dbg!("cursor: {:#?}", self.view.text_area.cursor());
          Some(Action::Update)
        },
        KeyEvent { code: KeyCode::Char('l'), .. } => {
          self.view.text_area.move_cursor(CursorMove::Forward);
          Some(Action::Update)
        },
        KeyEvent { code: KeyCode::Char('w'), .. } => {
          self.view.text_area.move_cursor(CursorMove::WordForward);
          Some(Action::Update)
        },
        KeyEvent { code: KeyCode::Char('b'), .. } => {
          self.view.text_area.move_cursor(CursorMove::WordBack);
          Some(Action::Update)
        },
        KeyEvent { code: KeyCode::Char('^'), .. } => {
          self.view.text_area.move_cursor(CursorMove::Head);
          Some(Action::Update)
        },
        KeyEvent { code: KeyCode::Char('$'), .. } => {
          self.view.text_area.move_cursor(CursorMove::End);
          Some(Action::Update)
        },
        KeyEvent { code: KeyCode::Char('v'), .. } => {
          self.view.text_area.start_selection();
          Some(Action::Update)
        },
        KeyEvent { code: KeyCode::Char('y'), .. } => {
          self.view.text_area.copy();
          let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
          ctx.set_contents(self.view.text_area.yank_text()).unwrap();
          Some(Action::Update)
        },
        KeyEvent { code: KeyCode::Esc, .. } => {
          self.view.text_area.cancel_selection();
          Some(Action::Update)
        },
        KeyEvent { code: KeyCode::Char('V'), modifiers: KeyModifiers::SHIFT, .. } => {
          self.view.text_area.start_selection();
          self.view.text_area.move_cursor(CursorMove::Head);
          self.view.text_area.start_selection();
          self.view.text_area.move_cursor(CursorMove::End);
          Some(Action::Update)
        },
        _ => None,
      },
      _ => None,
      //     KeyCode::Char('j') => self.scroll_down(),
      //     KeyCode::Char('k') => self.scroll_up(),
      //     _ => Ok(None),
      //   },
      //   _ => Ok(None),
    })
  }

  fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<(), SazidError> {
    let rects = Layout::default()
      .direction(Direction::Vertical)
      .constraints([Constraint::Percentage(100), Constraint::Min(self.input_vsize)].as_ref())
      .split(area);
    let inner = Layout::default()
      .direction(Direction::Vertical)
      .constraints(vec![Constraint::Length(1), Constraint::Min(10), Constraint::Length(0)])
      .split(rects[0]);
    // let inner = Layout::default()
    //   .direction(Direction::Horizontal)
    //   .constraints(vec![Constraint::Length(3), Constraint::Min(10), Constraint::Length(3)])
    //   .split(inner_a[1]);

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

    // let text = self.view.get_stylized_rendered_slice(
    //   self.vertical_scroll,
    //   self.vertical_viewport_height,
    //   self.vertical_scroll,
    //   self.get_select_coords(),
    // );

    // let paragraph = Paragraph::new(text.0.into_text().unwrap())
    //   .block(block)
    //   //.wrap(Wrap { trim: false })
    //   .scroll((self.vertical_scroll as u16, 0));
    // let scrollbar = Scrollbar::default()
    //   .orientation(ScrollbarOrientation::VerticalRight)
    //   .thumb_symbol("󱁨")
    //   .begin_symbol(Some("󰶼"))
    //   .end_symbol(Some("󰶹"));
    // f.render_widget(paragraph, inner[1]);
    f.render_widget(self.view.text_area.widget(), inner[1]);
    // f.render_stateful_widget(scrollbar, inner[2], &mut self.vertical_scroll_state);
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

  pub fn get_select_coords(&self) -> Option<((usize, usize), (usize, usize))> {
    match self.select_start_coords {
      Some((x1, y1)) => match self.select_end_coords {
        Some((x2, y2)) => Some(((x1, y1), (x2, y2))),
        None => None,
      },
      None => None,
    }
  }
  pub fn execute_tool_calls(&mut self) {
    let tx = self.action_tx.clone().unwrap();
    self
      .data
      .messages
      .iter_mut()
      .filter(|m| {
        m.receive_complete && !m.tools_called && matches!(m.message, ChatCompletionRequestMessage::Assistant(_))
      })
      .for_each(|m| {
        trace_dbg!("executing tool calls");
        if let ChatCompletionRequestMessage::Assistant(ChatCompletionRequestAssistantMessage {
          tool_calls: Some(tool_calls),
          ..
        }) = &m.message
        {
          tool_calls.iter().for_each(|tc| {
            let debug_text = format!("calling tool: {:?}", tc);
            trace_dbg!(level: tracing::Level::INFO, debug_text);
            handle_tool_call(tx.clone(), tc, self.config.clone());
          });
          m.tools_called = true;
        }
      })
  }

  fn redraw_messages(&mut self) {
    trace_dbg!("redrawing messages");
    self.data.messages.iter_mut().for_each(|m| {
      m.stylize_complete = false;
    });
    self.view.post_process_new_messages(&mut self.data);
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

  pub fn execute_command(&mut self, command: String) -> Result<String, SazidError> {
    let args = command.split_whitespace().collect::<Vec<&str>>();
    match args[0] {
      "exit" => std::process::exit(0),
      "load" => {
        if args.len() > 1 {
          self.load_session_by_id(args[1].to_string())?;
          Ok(format!("session {} loaded successfully!", args[1]))
        } else {
          self.load_last_session()?;
          Ok("last session loaded successfully!".to_string())
        }
      },
      _ => Ok("invalid command".to_string()),
    }
  }

  pub fn add_chunked_chat_completion_request_messages(
    &mut self,
    content: &str,
    _name: &str,
    role: Role,
    model: &Model,
  ) -> Result<(), SazidError> {
    match parse_input(content, CHUNK_TOKEN_LIMIT as usize, model.token_limit as usize) {
      Ok(chunks) => {
        chunks.iter().for_each(|chunk| {
          // explicitly calling update because we need this to be blocking, since it can't move on until the input is processed
          self
            .update(Action::AddMessage(ChatMessage::User(ChatCompletionRequestUserMessage {
              role,
              content: Some(ChatCompletionRequestUserMessageContent::Text(chunk.clone())),
            })))
            .unwrap();
        });
        Ok(())
      },
      Err(e) => Err(SazidError::ChunkifierError(e)),
    }
  }

  pub fn add_new_messages_to_request_buffer(&mut self) {
    // add new request messages to the request buffer
    let new_requests: Vec<ChatCompletionRequestMessage> = self
      .data
      .messages
      .iter()
      .skip(self.request_buffer.len())
      .filter(|m| m.receive_complete)
      .map(|m| {
        // let debug = format!("message: {:#?}", m.message).bright_red().to_string();
        // trace_dbg!(debug);
        m.message.clone()
      })
      .collect();
    self.request_buffer.extend(new_requests);
    trace_dbg!("request_buffer: {:#?}", self.request_buffer);
  }

  pub fn construct_request(&mut self) -> CreateChatCompletionRequest {
    let tools = match self.config.available_functions.is_empty() {
      true => None,
      false => {
        Some(create_chat_completion_tool_args(self.config.available_functions.iter().map(|f| f.into()).collect()))
      },
    };
    self.add_new_messages_to_request_buffer();
    let _token_count = 0;
    // let debug = format!("{:#?}", self.request_buffer).bright_cyan().to_string();
    // trace_dbg!("constructing request {}", debug);

    let request = CreateChatCompletionRequest {
      model: self.config.model.name.clone(),
      messages: self.request_buffer.clone().into_iter().collect(),
      stream: Some(self.config.stream_response),
      max_tokens: Some(self.config.response_max_tokens as u16),
      // todo: put the user information in here
      user: Some("testing testing".to_string()),
      tools,
      ..Default::default()
    };
    // trace_dbg!("request:\n{:#?}", request);
    request
  }

  fn filter_non_ascii(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii()).collect()
  }

  pub fn submit_chat_completion_request(&mut self, input: String, tx: UnboundedSender<Action>) {
    let config = self.config.clone();
    tx.send(Action::UpdateStatus(Some("submitting input".to_string()))).unwrap();
    match self.add_chunked_chat_completion_request_messages(
      Self::filter_non_ascii(&input).as_str(),
      config.name.as_str(),
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

  pub fn request_chat_completion(&mut self, tx: UnboundedSender<Action>) {
    tx.send(Action::UpdateStatus(Some("Configuring Client".to_string()))).unwrap();
    let stream_response = self.config.stream_response;
    let openai_config = self.config.openai_config.clone();

    let request = self.construct_request();
    debug_request_validation(&request);
    // let request = self.request_message_buffer.clone().unwrap();
    // let token_count = self.request_buffer_token_count;
    tx.send(Action::UpdateStatus(Some("Assembling request...".to_string()))).unwrap();
    tokio::spawn(async move {
      tx.send(Action::UpdateStatus(Some("Establishing Client Connection".to_string()))).unwrap();
      tx.send(Action::EnterProcessing).unwrap();
      let client = create_openai_client(&openai_config);
      trace_dbg!("client connection established");
      // tx.send(Action::AddMessage(ChatMessage::SazidSystemMessage(format!("Request Token Count: {}", token_count))))
      //   .unwrap();
      match stream_response {
        true => {
          tx.send(Action::UpdateStatus(Some("Sending Request to OpenAI API...".to_string()))).unwrap();
          trace_dbg!("Sending Request to API");
          let mut stream = client.chat().create_stream(request).await.unwrap();
          tx.send(Action::UpdateStatus(Some("Request submitted. Awaiting Response...".to_string()))).unwrap();
          while let Some(response_result) = stream.next().await {
            match response_result {
              Ok(response) => {
                trace_dbg!("Response: {:#?}", response.bright_yellow());
                //tx.send(Action::UpdateStatus(Some(format!("Received responses: {}", count).to_string()))).unwrap();
                tx.send(Action::AddMessage(ChatMessage::StreamResponse(vec![response]))).unwrap();
                tx.send(Action::Update).unwrap();
              },
              Err(e) => {
                trace_dbg!("Error: {:#?} -- check https://status.openai.com", e.bright_red());

                // let reqtext =
                //   format!("Request: \n{}", to_string_pretty(&request).unwrap_or("can't prettify result".to_string()));
                // trace_dbg!(&reqtext);
                // tx.send(Action::AddMessage(ChatMessage::SazidSystemMessage(reqtext))).unwrap();
                tx.send(Action::Error(format!("Error: {:?} -- check https://status.openai.com/", e))).unwrap();
              },
            }
          }
        },
        false => match client.chat().create(request).await {
          Ok(response) => {
            tx.send(Action::AddMessage(ChatMessage::Response(response))).unwrap();
            tx.send(Action::Update).unwrap();
          },
          Err(e) => {
            trace_dbg!("Error: {}", e);
            tx.send(Action::Error(format!("Error: {:#?} -- check https://status.openai.com/", e))).unwrap();
          },
        },
      };
      tx.send(Action::UpdateStatus(Some("Chat Request Complete".to_string()))).unwrap();
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

  fn load_session(&mut self, session_serde: String) -> Result<(), SazidError> {
    let incoming_session: Session = serde_json::from_str(session_serde.as_str()).unwrap();
    self.data = incoming_session.data;
    self.config = incoming_session.config;
    self.data.messages.iter_mut().for_each(|m| {
      m.stylize_complete = false;
    });
    Ok(())
  }
  pub fn load_session_by_id(&mut self, session_id: String) -> Result<(), SazidError> {
    Self::get_session_filepath(session_id.clone());
    let load_result = fs::read_to_string(Self::get_session_filepath(session_id.clone()));
    match load_result {
      Ok(load_session) => self.load_session(load_session),
      Err(e) => Err(SazidError::Other(format!("Failed to load session data: {:?}", e))),
    }
  }
  pub fn load_last_session(&mut self) -> Result<(), SazidError> {
    let home_dir = home_dir().unwrap();
    let save_dir = home_dir.join(SESSIONS_DIR);
    let session_files = list_files_ordered_by_date(save_dir).unwrap();
    let last_session_file = session_files.iter().last().unwrap();
    if last_session_file.path().is_file() {
      self.load_session_by_path(last_session_file.path().to_str().unwrap().to_string())
    } else {
      Err(SazidError::Other(format!("Failed to load session data: {:?}", last_session_file)))
    }
  }

  fn load_session_by_path(&mut self, session_file_path: String) -> Result<(), SazidError> {
    trace_dbg!("loading session from {}", session_file_path);

    let load_result = fs::read_to_string(session_file_path);
    match load_result {
      Ok(load_session) => self.load_session(load_session),
      Err(e) => Err(SazidError::Other(format!("Failed to load session data: {:?}", e))),
    }
  }
  fn save_session(&self) -> io::Result<()> {
    let home_dir = home_dir().unwrap();
    let save_dir = home_dir.join(SESSIONS_DIR);
    if !save_dir.exists() {
      fs::create_dir_all(save_dir.clone())?;
    }
    let session_file_path = save_dir.join(Self::get_session_filename(self.config.session_id.clone()));
    let data = serde_json::to_string(&self)?;
    fs::write(session_file_path.clone(), data)?;
    trace_dbg!("session saved to {}", &session_file_path.clone().display());
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
