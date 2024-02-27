use crate::{
  commands::{ApplyEditError, ApplyEditErrorKind},
  compositor::{Component, Context, EventResult},
};
use arc_swap::{
  access::{DynAccess, DynGuard},
  ArcSwap,
};
use async_openai::types::{
  ChatCompletionRequestAssistantMessage, ChatCompletionRequestMessage,
  ChatCompletionRequestUserMessage, ChatCompletionRequestUserMessageContent,
  ChatCompletionTool, CreateChatCompletionRequest, CreateEmbeddingRequestArgs,
  CreateEmbeddingResponse, Role,
};
use clipboard::{ClipboardContext, ClipboardProvider};
use color_eyre::owo_colors::OwoColorize;
use futures_util::stream::select_all::SelectAll;
use futures_util::{future, StreamExt};
use helix_lsp::{util::generate_transaction_from_edits, Call, OffsetEncoding};
use helix_view::{
  align_view,
  document::{
    DocumentSavedEventFuture, DocumentSavedEventResult, Mode, SavePoint,
  },
  editor::Action as EditorAction,
  editor::{
    CloseError, CursorShapeConfig, FilePickerConfig, PopupBorderConfig,
  },
  graphics::{CursorKind, Margin, Rect},
  handlers::Handlers,
  info::Info,
  input::{Event, KeyEvent, MouseButton, MouseEvent, MouseEventKind},
  register::Registers,
  theme::{self, Color, Style, Theme},
  tree::{self, Tree},
  view::ViewPosition,
  Align, Document, DocumentId, View, ViewId,
};
use ropey::Rope;
use tokio::{
  sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
  time::{sleep, Duration, Instant, Sleep},
};
use tokio_stream::wrappers::UnboundedReceiverStream;

use anyhow::{anyhow, bail, Error};

pub use helix_core::diagnostic::Severity;
use helix_core::{
  auto_pairs::AutoPairs,
  encoding::{self, Encoding},
  syntax::{
    self, AutoPairConfig, IndentationHeuristic, LanguageServerFeature, SoftWrap,
  },
  Change, ChangeSet, LineEnding, Position, Range, Selection,
  NATIVE_LINE_ENDING,
};
use helix_lsp::lsp;
use helix_stdx::path::canonicalize;

use std::{
  borrow::Cow,
  cell::Cell,
  collections::{BTreeMap, HashMap},
  default::Default,
  fs,
  io::{self, stdin},
  num::NonZeroUsize,
  path::{Path, PathBuf},
  pin::Pin,
  result::Result,
  sync::Arc,
};

use serde::{Deserialize, Serialize};
use tui::buffer::Buffer;
use tui::buffer::Buffer as Surface;
use tui::layout::{Constraint, Direction, Layout};
use tui::text::Text;
use tui::widgets::*;

use async_openai::{config::OpenAIConfig, Client};
use dotenv::dotenv;

use crate::app::database::data_manager::{
  get_all_embeddings_by_session, search_message_embeddings_by_session,
  DataManager,
};
use crate::app::database::types::QueryableSession;
use crate::app::functions::{all_functions, handle_tool_call};
use crate::app::markdown::Markdown;
use crate::app::messages::{
  ChatMessage, MessageContainer, MessageState, ReceiveBuffer,
};
use crate::app::request_validation::debug_request_validation;
use crate::app::session_config::SessionConfig;
use crate::app::session_view::SessionView;
use crate::app::{consts::*, errors::*, tools::chunkifier::*, types::*};
use crate::trace_dbg;
use crate::{action::Action, config::Config};
use backoff::exponential::ExponentialBackoffBuilder;

use futures_util::stream::{Flatten, Once};

use crate::app::gpt_interface::create_chat_completion_tool_args;
use crate::app::tools::utils::ensure_directory_exists;

pub struct Session {
  pub id: i64,
  pub messages: Vec<MessageContainer>,
  pub viewport_width: usize,
  pub session_config: SessionConfig,
  pub openai_config: OpenAIConfig,
  pub embeddings_manager: DataManager,
  pub view: SessionView,
  pub action_tx: Option<UnboundedSender<Action>>,
  pub mode: Mode,
  pub last_events: Vec<KeyEvent>,
  // pub vertical_scroll_state: ScrollbarState,
  // pub horizontal_scroll_state: ScrollbarState,
  pub vertical_scroll: usize,
  pub horizontal_scroll: usize,
  pub scroll_max: usize,
  pub vertical_content_height: usize,
  pub viewport_height: usize,
  pub scroll_sticky_end: bool,
  pub render: bool,
  pub fn_name: Option<String>,
  pub fn_args: Option<String>,
  pub request_buffer: Vec<ChatCompletionRequestMessage>,
  pub request_buffer_token_count: usize,
  pub input_vsize: u16,
  pub cursor_coords: Option<(usize, usize)>,
  pub select_start_coords: Option<(usize, usize)>,
  pub select_end_coords: Option<(usize, usize)>,

  pub next_document_id: DocumentId,
  pub documents: BTreeMap<DocumentId, Document>,

  pub tree: Tree,
  // We Flatten<> to resolve the inner DocumentSavedEventFuture. For that we need a stream of streams, hence the Once<>.
  // https://stackoverflow.com/a/66875668
  pub saves:
    HashMap<DocumentId, UnboundedSender<Once<DocumentSavedEventFuture>>>,
  pub save_queue:
    SelectAll<Flatten<UnboundedReceiverStream<Once<DocumentSavedEventFuture>>>>,
  pub write_count: usize,

  pub count: Option<std::num::NonZeroUsize>,
  pub selected_register: Option<char>,
  pub macro_recording: Option<(char, Vec<KeyEvent>)>,
  pub macro_replaying: Vec<char>,
  pub language_servers: helix_lsp::Registry,
  pub diagnostics: BTreeMap<PathBuf, Vec<(lsp::Diagnostic, usize)>>,

  pub file_picker: FilePickerConfig,
  pub syn_loader: Arc<ArcSwap<syntax::Loader>>,
  pub theme_loader: Arc<theme::Loader>,
  /// last_theme is used for theme previews. We store the current theme here,
  /// and if previewing is cancelled, we can return to it.
  pub last_theme: Option<Theme>,
  /// The currently applied editor theme. While previewing a theme, the previewed theme
  /// is set here.
  pub theme: Theme,

  /// The primary Selection prior to starting a goto_line_number preview. This is
  /// restored when the preview is aborted, or added to the jumplist when it is
  /// confirmed.
  pub last_selection: Option<Selection>,

  pub status_msg: Option<(Cow<'static, str>, Severity)>,
  pub autoinfo: Option<Info>,

  pub config: Arc<dyn DynAccess<SessionConfig>>,

  redraw_timer: Pin<Box<Sleep>>,
  last_motion: Option<Motion>,

  pub config_events:
    (UnboundedSender<ConfigEvent>, UnboundedReceiver<ConfigEvent>),
  pub needs_redraw: bool,
  /// Cached position of the cursor calculated during rendering.
  /// The content of `cursor_cache` is returned by `Editor::cursor` if
  /// set to `Some(_)`. The value will be cleared after it's used.
  /// If `cursor_cache` is `None` then the `Editor::cursor` function will
  /// calculate the cursor position.
  ///
  /// `Some(None)` represents a cursor position outside of the visible area.
  /// This will just cause `Editor::cursor` to return `None`.
  ///
  /// This cache is only a performance optimization to
  /// avoid calculating the cursor position multiple
  /// times during rendering and should not be set by other functions.
  pub cursor_cache: Cell<Option<Option<Position>>>,
  pub handlers: Handlers,

  pub mouse_down_range: Option<Range>,
}

pub type Motion = Box<dyn Fn(&mut Session)>;

#[derive(Debug, Clone)]
pub enum ConfigEvent {
  Refresh,
  Update(Box<Config>),
}

// impl From<QueryableSession> for Session {
//   fn from(value: QueryableSession) -> Self {
//     dotenv().ok();
//     let api_key = std::env::var("OPENAI_API_KEY").unwrap();
//     let openai_config = OpenAIConfig::new()
//       .with_api_key(api_key)
//       .with_org_id("org-WagBLu0vLgiuEL12dylmcPFj");
//     // let openai_config = OpenAIConfig::new().with_api_base("http://localhost:1234/v1".to_string());
//     Session {
//       id: value.id,
//       openai_config,
//       session_config: value.config.0,
//       ..Default::default()
//     }
//   }
// }

impl Component for Session {
  fn handle_event(
    &mut self,
    _event: &Event,
    _ctx: &mut Context,
  ) -> EventResult {
    EventResult::Ignored(None)
  }
  // , args: ()

  /// Should redraw? Useful for saving redraw cycles if we know component didn't change.
  fn should_update(&self) -> bool {
    true
  }

  /// Render the component onto the provided surface.
  fn render(&mut self, area: Rect, frame: &mut Surface, ctx: &mut Context) {}

  /// Get cursor position and cursor kind.
  fn cursor(
    &self,
    _area: Rect,
    _ctx: &Session,
  ) -> (Option<Position>, CursorKind) {
    (None, CursorKind::Hidden)
  }
  /// May be used by the parent component to compute the child area.
  /// viewport is the maximum allowed area, and the child should stay within those bounds.
  ///
  /// The returned size might be larger than the viewport if the child is too big to fit.
  /// In this case the parent can use the values to calculate scroll.
  fn required_size(&mut self, _viewport: (u16, u16)) -> Option<(u16, u16)> {
    None
  }

  fn type_name(&self) -> &'static str {
    std::any::type_name::<Self>()
  }

  fn id(&self) -> Option<&'static str> {
    None
  }
}

impl Session {
  pub fn new(
    mut area: Rect,
    theme_loader: Arc<theme::Loader>,
    syn_loader: Arc<ArcSwap<syntax::Loader>>,
    config: Arc<dyn DynAccess<SessionConfig>>,
    handlers: Handlers,
  ) -> Self {
    let language_servers = helix_lsp::Registry::new(syn_loader.clone());
    let conf = config.load();

    // HAXX: offset the render area height by 1 to account for prompt/commandline
    area.height -= 1;
    Session {
      id: 0,
      messages: vec![],
      tree: Tree::new(area),
      session_config: SessionConfig::default(),
      openai_config: OpenAIConfig::default(),
      embeddings_manager: DataManager::default(),
      action_tx: None,
      mode: Mode::Normal,
      last_events: vec![],
      // vertical_scroll_state: ScrollbarState::default(),
      view: SessionView::default(),
      // horizontal_scroll_state: ScrollbarState::default(),
      vertical_scroll: 0,
      scroll_max: 0,
      horizontal_scroll: 0,
      vertical_content_height: 0,
      file_picker: FilePickerConfig::default(),
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
      next_document_id: DocumentId::default(),
      documents: BTreeMap::new(),
      saves: HashMap::new(),
      save_queue: SelectAll::new(),
      write_count: 0,
      count: None,
      selected_register: None,
      macro_recording: None,
      macro_replaying: Vec::new(),
      theme: theme_loader.default(),
      language_servers,
      diagnostics: BTreeMap::new(),
      syn_loader,
      theme_loader,
      last_theme: None,
      last_selection: None,
      status_msg: None,
      autoinfo: None,
      redraw_timer: Box::pin(sleep(Duration::MAX)),
      last_motion: None,
      config,
      config_events: unbounded_channel(),
      needs_redraw: false,
      cursor_cache: Cell::new(None),
      handlers,
      mouse_down_range: None,
    }
  }
  fn init(&mut self, area: Rect) -> Result<(), SazidError> {
    let tx = self.action_tx.clone().unwrap();
    //let model_preference: Vec<Model> = vec![GPT4.clone(), GPT3_TURBO.clone(), WIZARDLM.clone()];
    //Session::select_model(model_preference, create_openai_client(self.openai_config.clone()));
    trace_dbg!("init session");
    self.session_config.prompt =
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
    tx.send(Action::AddMessage(ChatMessage::System(
      self.session_config.prompt_message(),
    )))
    .unwrap();
    self.view.post_process_new_messages(&mut self.messages);
    // self.text_area = TextArea::new(self.view.rendered_text.lines().map(|l| l.to_string()).collect());
    self.session_config.available_functions = all_functions();
    Ok(())
  }
  fn register_action_handler(
    &mut self,
    tx: UnboundedSender<Action>,
  ) -> Result<(), SazidError> {
    trace_dbg!("register_session_action_handler");
    self.action_tx = Some(tx);
    Ok(())
  }

  // fn register_config_handler(
  //   &mut self,
  //   config: Config,
  // ) -> Result<(), SazidError> {
  //   self.session_config = config.session_config;
  //   Ok(())
  // }

  fn update(&mut self, action: Action) -> Result<Option<Action>, SazidError> {
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
        self.submit_chat_completion_request(s, tx);
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
      Action::SelectModel(model) => self.session_config.model = model,
      Action::SetInputVsize(vsize) => {
        self.input_vsize = vsize;
      },
      // Action::EnterCommand => {
      //   self.view.unfocus_textarea();
      //   self.mode = Mode::Command;
      // },
      // Action::EnterNormal => {
      //   self.view.focus_textarea();
      //   self.mode = Mode::Normal;
      // },
      // Action::EnterVisual => {
      //   self.view.unfocus_textarea();
      //   self.mode = Mode::Visual;
      // },
      // Action::EnterInsert => {
      //   self.view.unfocus_textarea();
      //   self.mode = Mode::Insert;
      // },
      // Action::EnterProcessing => {
      //   self.view.unfocus_textarea();
      //   self.mode = Mode::Processing;
      // },
      // Action::ExitProcessing => {
      //   self.view.focus_textarea();
      //   self.mode = Mode::Normal;
      // },
      _ => (),
    }
    //self.action_tx.clone().unwrap().send(Action::Render).unwrap();
    Ok(None)
  }

  fn handle_events(
    &mut self,
    event: Option<Event>,
  ) -> Result<Option<Action>, SazidError> {
    let r = match event {
      Some(Event::Key(key_event)) => self.handle_key_events(key_event)?,
      Some(Event::Mouse(mouse_event)) => {
        self.handle_mouse_events(mouse_event)?
      },
      _ => None,
    };
    Ok(r)
  }

  fn handle_mouse_events(
    &mut self,
    mouse_event: MouseEvent,
  ) -> Result<Option<Action>, SazidError> {
    match mouse_event {
      MouseEvent { kind: MouseEventKind::ScrollUp, .. } => self.scroll_up(),
      MouseEvent { kind: MouseEventKind::ScrollDown, .. } => self.scroll_down(),
      MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column,
        row,
        modifiers,
      } => {
        // translate mouse click coordinates to text column and row
        self.select_end_coords = Some((column as usize, row as usize));
        // self.select_coords = Some((column as usize, row as usize));
        self.cursor_coords = Some((column as usize, row as usize));
        self.view.new_data = true;
        trace_dbg!(
          "mouse drag: column: {}, row: {}, modifiers: {:?}",
          column,
          row,
          modifiers
        );
        Ok(Some(Action::Update))
      },
      MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        ..
      } => {
        // translate mouse click coordinates to text column and row
        self.select_start_coords = Some((column as usize, row as usize));
        self.cursor_coords = Some((column as usize, row as usize));
        self.view.new_data = true;
        Ok(Some(Action::Update))
      },
      _ => Ok(None),
    }
  }

  fn handle_key_events(
    &mut self,
    key: KeyEvent,
  ) -> Result<Option<Action>, SazidError> {
    self.last_events.push(key);
    Ok(match self.mode {
      Mode::Normal => match key {
        // KeyEvent {
        //   code: KeyCode::Char('d'),
        //   modifiers: KeyModifiers::CONTROL,
        //   ..
        // } => {
        //   self.view.textarea.scroll(Scrolling::HalfPageDown);
        //   Some(Action::Update)
        // },
        // KeyEvent {
        //   code: KeyCode::Char('u'),
        //   modifiers: KeyModifiers::CONTROL,
        //   ..
        // } => {
        //   self.view.textarea.scroll(Scrolling::HalfPageUp);
        //   Some(Action::Update)
        // },
        // KeyEvent {
        //   code: KeyCode::Char('f'),
        //   modifiers: KeyModifiers::CONTROL,
        //   ..
        // } => {
        //   self.view.textarea.scroll(Scrolling::PageDown);
        //   self.scroll_sticky_end =
        //     self.view.textarea.cursor().0 == self.view.textarea.lines().len();
        //   Some(Action::Update)
        // },
        // KeyEvent {
        //   code: KeyCode::Char('b'),
        //   modifiers: KeyModifiers::CONTROL,
        //   ..
        // } => {
        //   self.view.textarea.scroll(Scrolling::PageUp);
        //   Some(Action::Update)
        // },
        // KeyEvent { code: KeyCode::Char('h'), .. } => {
        //   self.view.textarea.move_cursor(CursorMove::Back);
        //   Some(Action::Update)
        // },
        // KeyEvent { code: KeyCode::Char('j'), .. } => {
        //   self.view.textarea.move_cursor(CursorMove::Down);
        //   self.scroll_sticky_end =
        //     self.view.textarea.cursor().0 == self.view.textarea.lines().len();
        //   trace_dbg!("cursor: {:#?}", self.view.textarea.cursor());
        //   Some(Action::Update)
        // },
        // KeyEvent { code: KeyCode::Char('k'), .. } => {
        //   self.view.textarea.move_cursor(CursorMove::Up);
        //   trace_dbg!("cursor: {:#?}", self.view.textarea.cursor());
        //   Some(Action::Update)
        // },
        // KeyEvent { code: KeyCode::Char('l'), .. } => {
        //   self.view.textarea.move_cursor(CursorMove::Forward);
        //   Some(Action::Update)
        // },
        // KeyEvent { code: KeyCode::Char('w'), .. } => {
        //   self.view.textarea.move_cursor(CursorMove::WordForward);
        //   Some(Action::Update)
        // },
        // KeyEvent { code: KeyCode::Char('b'), .. } => {
        //   self.view.textarea.move_cursor(CursorMove::WordBack);
        //   Some(Action::Update)
        // },
        // KeyEvent { code: KeyCode::Char('^'), .. } => {
        //   self.view.textarea.move_cursor(CursorMove::Head);
        //   Some(Action::Update)
        // },
        // KeyEvent { code: KeyCode::Char('$'), .. } => {
        //   self.view.textarea.move_cursor(CursorMove::End);
        //   self.scroll_sticky_end =
        //     self.view.textarea.cursor().0 == self.view.textarea.lines().len();
        //   Some(Action::Update)
        // },
        // KeyEvent { code: KeyCode::Char('v'), .. } => {
        //   self.view.textarea.start_selection();
        //   Some(Action::Update)
        // },
        // KeyEvent { code: KeyCode::Char('y'), .. } => {
        //   self.view.textarea.copy();
        //   let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
        //   ctx
        //     .set_contents(ansi_to_plain_text(&self.view.textarea.yank_text()))
        //     .unwrap();
        //   Some(Action::Update)
        // },
        // KeyEvent { code: KeyCode::Esc, .. } => {
        //   self.view.textarea.cancel_selection();
        //   Some(Action::Update)
        // },
        // KeyEvent {
        //   code: KeyCode::Char('V'),
        //   modifiers: KeyModifiers::SHIFT,
        //   ..
        // } => {
        //   self.view.textarea.start_selection();
        //   self.view.textarea.move_cursor(CursorMove::Head);
        //   self.view.textarea.start_selection();
        //   self.view.textarea.move_cursor(CursorMove::End);
        //   Some(Action::Update)
        // },
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

  fn draw(&mut self, b: &mut Buffer) -> Result<(), SazidError> {
    let margin_width;
    let session_width;
    if b.area.width <= 81 {
      margin_width = 0u16;
      session_width = b.area.width
    } else {
      session_width = 82;
      margin_width = (b.area.width - session_width) / 2;
    }
    let rects = Layout::default()
      .direction(Direction::Vertical)
      .constraints(
        [Constraint::Percentage(100), Constraint::Min(self.input_vsize)]
          .as_ref(),
      )
      .split(b.area);
    let inner = Layout::default()
      .direction(Direction::Vertical)
      .constraints(vec![
        Constraint::Length(1),
        Constraint::Min(10),
        Constraint::Length(0),
      ])
      .split(rects[0]);
    let inner = Layout::default()
      .direction(Direction::Horizontal)
      .constraints(vec![
        Constraint::Length(margin_width),
        Constraint::Length(session_width),
        Constraint::Length(margin_width),
      ])
      .split(inner[1]);

    let block = Block::default()
      .borders(Borders::ALL)
      .border_style(Style::default().fg(Color::Gray));

    let debug_text = Paragraph::new(format!(
      "--debug--\nsticky end: {}\nscroll: {}\ncontent height: {}\nviewport height: {}",
      self.scroll_sticky_end, self.vertical_scroll, self.vertical_content_height, self.viewport_height
    ))
    .block(block);
    self.viewport_height = inner[1].height as usize;
    self.vertical_content_height = self.view.rendered_text.len_lines();
    // self.vertical_scroll_state =
    //   self.vertical_scroll_state.content_length(self.vertical_content_height);
    self.view.set_window_width(session_width as usize, &mut self.messages);
    self.scroll_max =
      self.view.rendered_text.len_lines().saturating_sub(self.viewport_height);

    if self.scroll_sticky_end {
      // self.view.textarea.move_cursor(CursorMove::Bottom);
      // self.view.textarea.move_cursor(CursorMove::Head);
    }
    debug_text.render(inner[0], b);
    // f.render_widget(self.view.textarea.widget(), inner[1]);
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
//              --content-- <- scroll_top
//
//              --content-- <- scroll_top +rendered_line_count
// --window-- <- vertical_scroll
//
//
// --window-- <- vertical_scroll + vertical_viewport_height
//
//
//              --content-- <- scroll_top
// --window-- <- vertical_scroll
//                                 message_line_start_index
//              --content-- <- scroll_top +rendered_line_count
//                                 message_line_last_index
// --window-- <- vertical_scroll + vertical_viewport_height
//
//
// --window-- <- vertical_scroll
//              --content-- <- scroll_top
//              --content-- <- scroll_top +rendered_line_count
// --window-- <- vertical_scroll + vertical_viewport_height
//
//
// --window-- <- vertical_scroll
//              --content-- <- scroll_top
// --window-- <- vertical_scroll + vertical_viewport_height
//              --content-- <- scroll_top +rendered_line_count
//
// --window-- <- vertical_scroll
// --window-- <- vertical_scroll + vertical_viewport_height
//              --content-- <- scroll_top
//              --content-- <- scroll_top +rendered_line_count
impl Session {
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
          Arc::clone(&self.view.lang_config),
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
        let content = format!("{}", m);
        let markdown = Markdown::new(
          content,
          self.view.window_width,
          Arc::clone(&self.view.lang_config),
        );

        let text = markdown.parse(None)
          [message_line_start_index..message_line_last_index]
          .to_vec();
        let par = Paragraph::new(Text::from(text))
          .wrap(Wrap { trim: false })
          .scroll((scroll as u16, 0));
        let margin = Margin::all(1);
        par.render(area.inner(&margin), surface);
      });
  }

  pub fn add_message(&mut self, message: ChatMessage) {
    match message {
      ChatMessage::User(_) => {
        let mut message = MessageContainer::from(message);
        message.set_current_transaction_flag();
        self.messages.push(message);
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
            message.update_stream_response(sr.clone()).unwrap();
          } else {
            let mut message: MessageContainer =
              ChatMessage::StreamResponse(vec![sr.clone()]).into();
            message.set_current_transaction_flag();
            self.messages.push(message);
          }
        });
      },
      _ => {
        let mut message: MessageContainer = message.into();
        message.set_current_transaction_flag();
        self.messages.push(message);
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
            handle_tool_call(tx.clone(), tc, self.session_config.clone());
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
            content: Some(ChatCompletionRequestUserMessageContent::Text(
              chunk.clone(),
            )),
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

  pub fn submit_chat_completion_request(
    &mut self,
    input: String,
    tx: UnboundedSender<Action>,
  ) {
    let config = self.session_config.clone();
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
    let stream_response = self.session_config.stream_response;
    let openai_config = self.openai_config.clone();
    let db_url = self.embeddings_manager.get_database_url();
    let model = self.session_config.model.clone();
    let embedding_model = self.embeddings_manager.model.clone();
    let user = self.session_config.user.clone();
    let session_id = self.id;
    let max_tokens = self.session_config.response_max_tokens;
    let rag = self.session_config.retrieval_augmentation_message_count;
    let stream = Some(self.session_config.stream_response);

    let tools = match self.session_config.available_functions.is_empty() {
      true => None,
      false => Some(create_chat_completion_tool_args(
        self
          .session_config
          .available_functions
          .iter()
          .map(|f| f.into())
          .collect(),
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

  /// Current editing mode for the [`Editor`].
  pub fn mode(&self) -> Mode {
    self.mode
  }

  pub fn config(&self) -> DynGuard<SessionConfig> {
    self.config.load()
  }

  pub fn clear_status(&mut self) {
    self.status_msg = None;
  }

  #[inline]
  pub fn set_status<T: Into<Cow<'static, str>>>(&mut self, status: T) {
    let status = status.into();
    log::debug!("editor status: {}", status);
    self.status_msg = Some((status, Severity::Info));
  }

  #[inline]
  pub fn set_error<T: Into<Cow<'static, str>>>(&mut self, error: T) {
    let error = error.into();
    log::debug!("editor error: {}", error);
    self.status_msg = Some((error, Severity::Error));
  }

  #[inline]
  pub fn get_status(&self) -> Option<(&Cow<'static, str>, &Severity)> {
    self.status_msg.as_ref().map(|(status, sev)| (status, sev))
  }

  /// Returns true if the current status is an error
  #[inline]
  pub fn is_err(&self) -> bool {
    self
      .status_msg
      .as_ref()
      .map(|(_, sev)| *sev == Severity::Error)
      .unwrap_or(false)
  }

  /// Gets the primary cursor position in screen coordinates,
  /// or `None` if the primary cursor is not visible on screen.
  pub fn cursor(&self) -> (Option<Position>, CursorKind) {
    let config = self.config();
    let (view, doc) = current_ref!(self);
    let cursor = doc.selection(view.id).primary().cursor(doc.text().slice(..));
    let pos = self.cursor_cache.get().unwrap_or_else(|| {
      view.screen_coords_at_pos(doc, doc.text().slice(..), cursor)
    });
    if let Some(mut pos) = pos {
      let inner = view.inner_area(doc);
      pos.col += inner.x as usize;
      pos.row += inner.y as usize;
      let cursorkind = config.cursor_shape.from_mode(self.mode);
      (Some(pos), cursorkind)
    } else {
      (None, CursorKind::default())
    }
  }

  fn replace_document_in_view(
    &mut self,
    current_view: ViewId,
    doc_id: DocumentId,
  ) {
    let view = self.tree.get_mut(current_view);
    view.doc = doc_id;
    view.offset = ViewPosition::default();

    let doc = doc_mut!(self, &doc_id);
    doc.ensure_view_init(view.id);
    view.sync_changes(doc);
    doc.mark_as_focused();

    align_view(doc, view, Align::Center);
  }

  pub async fn flush_writes(&mut self) -> anyhow::Result<()> {
    while self.write_count > 0 {
      if let Some(save_event) = self.save_queue.next().await {
        self.write_count -= 1;

        let save_event = match save_event {
          Ok(event) => event,
          Err(err) => {
            self.set_error(err.to_string());
            bail!(err);
          },
        };

        let doc = doc_mut!(self, &save_event.doc_id);
        doc.set_last_saved_revision(save_event.revision);
      }
    }

    Ok(())
  }

  pub fn popup_border(&self) -> bool {
    self.config().popup_border == PopupBorderConfig::All
      || self.config().popup_border == PopupBorderConfig::Popup
  }

  pub fn menu_border(&self) -> bool {
    self.config().popup_border == PopupBorderConfig::All
      || self.config().popup_border == PopupBorderConfig::Menu
  }

  pub fn apply_motion<F: Fn(&mut Self) + 'static>(&mut self, motion: F) {
    motion(self);
    self.last_motion = Some(Box::new(motion));
  }

  pub fn repeat_last_motion(&mut self, count: usize) {
    if let Some(motion) = self.last_motion.take() {
      for _ in 0..count {
        motion(self);
      }
      self.last_motion = Some(motion);
    }
  }
  /// Switches the editor into normal mode.
  pub fn enter_normal_mode(&mut self) {
    use helix_core::graphemes;

    if self.mode == Mode::Normal {
      return;
    }

    self.mode = Mode::Normal;
    let (view, doc) = current!(self);

    try_restore_indent(doc, view);

    // if leaving append mode, move cursor back by 1
    if doc.restore_cursor {
      let text = doc.text().slice(..);
      let selection = doc.selection(view.id).clone().transform(|range| {
        let mut head = range.to();
        if range.head > range.anchor {
          head = graphemes::prev_grapheme_boundary(text, head);
        }

        Range::new(range.from(), head)
      });

      doc.set_selection(view.id, selection);
      doc.restore_cursor = false;
    }
  }

  fn _refresh(&mut self) {
    let config = self.config();

    // Reset the inlay hints annotations *before* updating the views, that way we ensure they
    // will disappear during the `.sync_change(doc)` call below.
    //
    // We can't simply check this config when rendering because inlay hints are only parts of
    // the possible annotations, and others could still be active, so we need to selectively
    // drop the inlay hints.
    if !config.lsp.display_inlay_hints {
      for doc in self.documents_mut() {
        doc.reset_all_inlay_hints();
      }
    }

    for (view, _) in self.tree.views_mut() {
      let doc = doc_mut!(self, &view.doc);
      view.sync_changes(doc);
      view.gutters = config.gutters.clone();
      view.ensure_cursor_in_view(doc, config.scrolloff)
    }
  }

  pub fn switch(&mut self, id: DocumentId, action: EditorAction) {
    use helix_view::tree::Layout;
    if !self.documents.contains_key(&id) {
      log::error!("cannot switch to document that does not exist (anymore)");
      return;
    }

    self.enter_normal_mode();

    match action {
      EditorAction::Replace => {
        let (view, doc) = current_ref!(self);
        // If the current view is an empty scratch buffer and is not displayed in any other views, delete it.
        // Boolean value is determined before the call to `view_mut` because the operation requires a borrow
        // of `self.tree`, which is mutably borrowed when `view_mut` is called.
        let remove_empty_scratch = !doc.is_modified()
                    // If the buffer has no path and is not modified, it is an empty scratch buffer.
                    && doc.path().is_none()
                    // If the buffer we are changing to is not this buffer
                    && id != doc.id()
                    // Ensure the buffer is not displayed in any other splits.
                    && !self
                        .tree
                        .traverse()
                        .any(|(_, v)| v.doc == doc.id() && v.id != view.id);

        let (view, doc) = current!(self);
        let view_id = view.id;

        // Append any outstanding changes to history in the old document.
        doc.append_changes_to_history(view);

        if remove_empty_scratch {
          // Copy `doc.id` into a variable before calling `self.documents.remove`, which requires a mutable
          // borrow, invalidating direct access to `doc.id`.
          let id = doc.id();
          self.documents.remove(&id);

          // Remove the scratch buffer from any jumplists
          for (view, _) in self.tree.views_mut() {
            view.remove_document(&id);
          }
        } else {
          let jump = (view.doc, doc.selection(view.id).clone());
          view.jumps.push(jump);
          // Set last accessed doc if it is a different document
          if doc.id() != id {
            view.add_to_history(view.doc);
            // Set last modified doc if modified and last modified doc is different
            if std::mem::take(&mut doc.modified_since_accessed)
              && view.last_modified_docs[0] != Some(view.doc)
            {
              view.last_modified_docs =
                [Some(view.doc), view.last_modified_docs[0]];
            }
          }
        }

        self.replace_document_in_view(view_id, id);

        return;
      },
      EditorAction::Load => {
        let view_id = view!(self).id;
        let doc = doc_mut!(self, &id);
        doc.ensure_view_init(view_id);
        doc.mark_as_focused();
        return;
      },
      EditorAction::HorizontalSplit | EditorAction::VerticalSplit => {
        // copy the current view, unless there is no view yet
        let view = self
                    .tree
                    .try_get(self.tree.focus)
                    .filter(|v| id == v.doc) // Different Document
                    .cloned()
                    .unwrap_or_else(|| View::new(id, self.config().gutters.clone()));
        let view_id = self.tree.split(
          view,
          match action {
            EditorAction::HorizontalSplit => Layout::Horizontal,
            EditorAction::VerticalSplit => Layout::Vertical,
            _ => unreachable!(),
          },
        );
        // initialize selection for view
        let doc = doc_mut!(self, &id);
        doc.ensure_view_init(view_id);
        doc.mark_as_focused();
      },
    }

    self._refresh();
  }
  pub fn close_document(
    &mut self,
    doc_id: DocumentId,
    force: bool,
  ) -> Result<(), CloseError> {
    let doc = match self.documents.get_mut(&doc_id) {
      Some(doc) => doc,
      None => return Err(CloseError::DoesNotExist),
    };
    if !force && doc.is_modified() {
      return Err(CloseError::BufferModified(doc.display_name().into_owned()));
    }

    // This will also disallow any follow-up writes
    self.saves.remove(&doc_id);

    for language_server in doc.language_servers() {
      // TODO: track error
      tokio::spawn(language_server.text_document_did_close(doc.identifier()));
    }

    enum Action {
      Close(ViewId),
      ReplaceDoc(ViewId, DocumentId),
    }

    let actions: Vec<Action> = self
      .tree
      .views_mut()
      .filter_map(|(view, _focus)| {
        view.remove_document(&doc_id);

        if view.doc == doc_id {
          // something was previously open in the view, switch to previous doc
          if let Some(prev_doc) = view.docs_access_history.pop() {
            Some(Action::ReplaceDoc(view.id, prev_doc))
          } else {
            // only the document that is being closed was in the view, close it
            Some(Action::Close(view.id))
          }
        } else {
          None
        }
      })
      .collect();

    for action in actions {
      match action {
        Action::Close(view_id) => {
          self.close(view_id);
        },
        Action::ReplaceDoc(view_id, doc_id) => {
          self.replace_document_in_view(view_id, doc_id);
        },
      }
    }

    self.documents.remove(&doc_id);

    // If the document we removed was visible in all views, we will have no more views. We don't
    // want to close the editor just for a simple buffer close, so we need to create a new view
    // containing either an existing document, or a brand new document.
    if self.tree.views().next().is_none() {
      let doc_id =
        self.documents.iter().map(|(&doc_id, _)| doc_id).next().unwrap_or_else(
          || self.new_document(Document::default(self.config.clone())),
        );
      let view = View::new(doc_id, self.config().gutters.clone());
      let view_id = self.tree.insert(view);
      let doc = doc_mut!(self, &doc_id);
      doc.ensure_view_init(view_id);
      doc.mark_as_focused();
    }

    self._refresh();

    Ok(())
  }

  pub fn open(
    &mut self,
    path: &Path,
    action: EditorAction,
  ) -> Result<DocumentId, Error> {
    let path = helix_stdx::path::canonicalize(path);
    let id = self.document_by_path(&path).map(|doc| doc.id());

    let id = if let Some(id) = id {
      id
    } else {
      let mut doc = Document::open(
        &path,
        None,
        Some(self.syn_loader.clone()),
        self.config.clone(),
      )?;

      let diagnostics = Session::doc_diagnostics(
        &self.language_servers,
        &self.diagnostics,
        &doc,
      );
      doc.replace_diagnostics(diagnostics, &[], None);

      if let Some(diff_base) = self.diff_providers.get_diff_base(&path) {
        doc.set_diff_base(diff_base);
      }
      doc.set_version_control_head(
        self.diff_providers.get_current_head_name(&path),
      );

      let id = self.new_document(doc);
      self.launch_language_servers(id);

      id
    };

    self.switch(id, action);
    Ok(id)
  }

  /// Launch a language server for a given document
  fn launch_language_servers(&mut self, doc_id: DocumentId) {
    if !self.config().lsp.enable {
      return;
    }
    // if doc doesn't have a URL it's a scratch buffer, ignore it
    let Some(doc) = self.documents.get_mut(&doc_id) else {
      return;
    };
    let Some(doc_url) = doc.url() else {
      return;
    };
    let (lang, path) = (doc.language.clone(), doc.path().cloned());
    let config = doc.config.load();
    let root_dirs = &config.workspace_lsp_roots;

    // store only successfully started language servers
    let language_servers = lang.as_ref().map_or_else(HashMap::default, |language| {
            self.language_servers
                .get(language, path.as_ref(), root_dirs, config.lsp.snippets)
                .filter_map(|(lang, client)| match client {
                    Ok(client) => Some((lang, client)),
                    Err(err) => {
                        if let helix_lsp::Error::ExecutableNotFound(err) = err {
                            // Silence by default since some language servers might just not be installed
                            log::debug!(
                                "Language server not found for `{}` {} {}", language.scope(), lang, err,
                            );
                        } else {
                            log::error!(
                                "Failed to initialize the language servers for `{}` - `{}` {{ {} }}",
                                language.scope(),
                                lang,
                                err
                            );
                        }
                        None
                    }
                })
                .collect::<HashMap<_, _>>()
        });

    if language_servers.is_empty() && doc.language_servers().count() == 0 {
      return;
    }

    let language_id =
      doc.language_id().map(ToOwned::to_owned).unwrap_or_default();

    // only spawn new language servers if the servers aren't the same
    let doc_language_servers_not_in_registry =
      doc.language_servers().iter().filter(|(name, doc_ls)| {
        language_servers.get(*name).map_or(true, |ls| ls.id() != doc_ls.id())
      });

    for (_, language_server) in doc_language_servers_not_in_registry {
      tokio::spawn(language_server.text_document_did_close(doc.identifier()));
    }

    let language_servers_not_in_doc =
      language_servers.iter().filter(|(name, ls)| {
        doc
          .language_servers()
          .get(*name)
          .map_or(true, |doc_ls| ls.id() != doc_ls.id())
      });

    for (_, language_server) in language_servers_not_in_doc {
      // TODO: this now races with on_init code if the init happens too quickly
      tokio::spawn(language_server.text_document_did_open(
        doc_url.clone(),
        doc.version(),
        doc.text(),
        language_id.clone(),
      ));
    }

    doc.language_servers() = language_servers;
  }

  pub fn close(&mut self, id: ViewId) {
    // Remove selections for the closed view on all documents.
    for doc in self.documents_mut() {
      doc.remove_view(id);
    }
    self.tree.remove(id);
    self._refresh();
  }

  /// Generate an id for a new document and register it.
  fn new_document(&mut self, mut doc: Document) -> DocumentId {
    let id = self.next_document_id;
    // Safety: adding 1 from 1 is fine, probably impossible to reach usize max
    self.next_document_id = DocumentId(unsafe {
      NonZeroUsize::new_unchecked(self.next_document_id().0.get() + 1)
    });
    doc.id() = id;
    self.documents.insert(id, doc);

    let (save_sender, save_receiver) = tokio::sync::mpsc::unbounded_channel();
    self.saves.insert(id, save_sender);

    let stream = UnboundedReceiverStream::new(save_receiver).flatten();
    self.save_queue.push(stream);

    id
  }

  fn new_file_from_document(
    &mut self,
    action: EditorAction,
    doc: Document,
  ) -> DocumentId {
    let id = self.new_document(doc);
    self.switch(id, action);
    id
  }

  pub fn new_file(&mut self, action: EditorAction) -> DocumentId {
    self.new_file_from_document(action, Document::default(self.config.clone()))
  }
  pub fn save<P: Into<PathBuf>>(
    &mut self,
    doc_id: DocumentId,
    path: Option<P>,
    force: bool,
  ) -> anyhow::Result<()> {
    // convert a channel of futures to pipe into main queue one by one
    // via stream.then() ? then push into main future

    let path = path.map(|path| path.into());
    let doc = doc_mut!(self, &doc_id);
    let doc_save_future = doc.save(path, force)?;

    // When a file is written to, notify the file event handler.
    // Note: This can be removed once proper file watching is implemented.
    let handler = self.language_servers.file_event_handler.clone();
    let future = async move {
      let res = doc_save_future.await;
      if let Ok(event) = &res {
        handler.file_changed(event.path.clone());
      }
      res
    };

    use futures_util::stream;

    self
      .saves
      .get(&doc_id)
      .ok_or_else(|| {
        anyhow::format_err!("saves are closed for this document!")
      })?
      .send(stream::once(Box::pin(future)))
      .map_err(|err| anyhow!("failed to send save event: {}", err))?;

    self.write_count += 1;

    Ok(())
  }

  pub fn resize(&mut self, area: Rect) {
    if self.tree.resize(area) {
      self._refresh();
    };
  }

  pub fn focus(&mut self, view_id: ViewId) {
    let prev_id = std::mem::replace(&mut self.tree.focus, view_id);

    // if leaving the view: mode should reset and the cursor should be
    // within view
    if prev_id != view_id {
      self.enter_normal_mode();
      self.ensure_cursor_in_view(view_id);

      // Update jumplist selections with new document changes.
      for (view, _focused) in self.tree.views_mut() {
        let doc = doc_mut!(self, &view.doc);
        view.sync_changes(doc);
      }
    }

    let view = view!(self, view_id);
    let doc = doc_mut!(self, &view.doc);
    doc.mark_as_focused();
  }

  pub fn focus_next(&mut self) {
    self.focus(self.tree.next());
  }

  pub fn focus_prev(&mut self) {
    self.focus(self.tree.prev());
  }

  pub fn focus_direction(&mut self, direction: tree::Direction) {
    let current_view = self.tree.focus;
    if let Some(id) = self.tree.find_split_in_direction(current_view, direction)
    {
      self.focus(id)
    }
  }

  pub fn swap_split_in_direction(&mut self, direction: tree::Direction) {
    self.tree.swap_split_in_direction(direction);
  }

  pub fn transpose_view(&mut self) {
    self.tree.transpose();
  }

  pub fn should_close(&self) -> bool {
    self.tree.is_empty()
  }

  pub fn ensure_cursor_in_view(&mut self, id: ViewId) {
    let config = self.config();
    let view = self.tree.get_mut(id);
    let doc = &self.documents[&view.doc];
    view.ensure_cursor_in_view(doc, config.scrolloff)
  }

  #[inline]
  pub fn document(&self, id: DocumentId) -> Option<&Document> {
    self.documents.get(&id)
  }

  #[inline]
  pub fn document_mut(&mut self, id: DocumentId) -> Option<&mut Document> {
    self.documents.get_mut(&id)
  }

  #[inline]
  pub fn documents(&self) -> impl Iterator<Item = &Document> {
    self.documents.values()
  }

  #[inline]
  pub fn documents_mut(&mut self) -> impl Iterator<Item = &mut Document> {
    self.documents.values_mut()
  }

  pub fn document_by_path<P: AsRef<Path>>(&self, path: P) -> Option<&Document> {
    self
      .documents()
      .find(|doc| doc.path().map(|p| p == path.as_ref()).unwrap_or(false))
  }

  pub fn document_by_path_mut<P: AsRef<Path>>(
    &mut self,
    path: P,
  ) -> Option<&mut Document> {
    self
      .documents_mut()
      .find(|doc| doc.path().map(|p| p == path.as_ref()).unwrap_or(false))
  }

  /// Returns all supported diagnostics for the document
  pub fn doc_diagnostics<'a>(
    language_servers: &'a helix_lsp::Registry,
    diagnostics: &'a BTreeMap<PathBuf, Vec<(lsp::Diagnostic, usize)>>,
    document: &Document,
  ) -> impl Iterator<Item = helix_core::Diagnostic> + 'a {
    Session::doc_diagnostics_with_filter(
      language_servers,
      diagnostics,
      document,
      |_, _| true,
    )
  }

  fn apply_text_edits(
    &mut self,
    uri: &helix_lsp::Url,
    version: Option<i32>,
    text_edits: Vec<lsp::TextEdit>,
    offset_encoding: OffsetEncoding,
  ) -> Result<(), ApplyEditErrorKind> {
    let path = match uri.to_file_path() {
      Ok(path) => path,
      Err(_) => {
        let err = format!("unable to convert URI to filepath: {}", uri);
        log::error!("{}", err);
        self.set_error(err);
        return Err(ApplyEditErrorKind::UnknownURISchema);
      },
    };

    let doc_id = match self.open(&path, Action::Load) {
      Ok(doc_id) => doc_id,
      Err(err) => {
        let err = format!("failed to open document: {}: {}", uri, err);
        log::error!("{}", err);
        self.set_error(err);
        return Err(ApplyEditErrorKind::FileNotFound);
      },
    };

    let doc = doc_mut!(self, &doc_id);
    if let Some(version) = version {
      if version != doc.version() {
        let err = format!("outdated workspace edit for {path:?}");
        log::error!("{err}, expected {} but got {version}", doc.version());
        self.set_error(err);
        return Err(ApplyEditErrorKind::DocumentChanged);
      }
    }

    // Need to determine a view for apply/append_changes_to_history
    let view_id = self.get_synced_view_id(doc_id);
    let doc = doc_mut!(self, &doc_id);

    let transaction =
      generate_transaction_from_edits(doc.text(), text_edits, offset_encoding);
    let view = view_mut!(self, view_id);
    doc.apply(&transaction, view.id);
    doc.append_changes_to_history(view);
    Ok(())
  }

  // TODO make this transactional (and set failureMode to transactional)
  pub fn apply_workspace_edit(
    &mut self,
    offset_encoding: OffsetEncoding,
    workspace_edit: &lsp::WorkspaceEdit,
  ) -> Result<(), ApplyEditError> {
    if let Some(ref document_changes) = workspace_edit.document_changes {
      match document_changes {
        lsp::DocumentChanges::Edits(document_edits) => {
          for (i, document_edit) in document_edits.iter().enumerate() {
            let edits = document_edit
              .edits
              .iter()
              .map(|edit| match edit {
                lsp::OneOf::Left(text_edit) => text_edit,
                lsp::OneOf::Right(annotated_text_edit) => {
                  &annotated_text_edit.text_edit
                },
              })
              .cloned()
              .collect();
            self
              .apply_text_edits(
                &document_edit.text_document.uri,
                document_edit.text_document.version,
                edits,
                offset_encoding,
              )
              .map_err(|kind| ApplyEditError { kind, failed_change_idx: i })?;
          }
        },
        lsp::DocumentChanges::Operations(operations) => {
          log::debug!("document changes - operations: {:?}", operations);
          for (i, operation) in operations.iter().enumerate() {
            match operation {
              lsp::DocumentChangeOperation::Op(op) => {
                self.apply_document_resource_op(op).map_err(|io| {
                  ApplyEditError {
                    kind: ApplyEditErrorKind::IoError(io),
                    failed_change_idx: i,
                  }
                })?;
              },

              lsp::DocumentChangeOperation::Edit(document_edit) => {
                let edits = document_edit
                  .edits
                  .iter()
                  .map(|edit| match edit {
                    lsp::OneOf::Left(text_edit) => text_edit,
                    lsp::OneOf::Right(annotated_text_edit) => {
                      &annotated_text_edit.text_edit
                    },
                  })
                  .cloned()
                  .collect();
                self
                  .apply_text_edits(
                    &document_edit.text_document.uri,
                    document_edit.text_document.version,
                    edits,
                    offset_encoding,
                  )
                  .map_err(|kind| ApplyEditError {
                    kind,
                    failed_change_idx: i,
                  })?;
              },
            }
          }
        },
      }

      return Ok(());
    }

    if let Some(ref changes) = workspace_edit.changes {
      log::debug!("workspace changes: {:?}", changes);
      for (i, (uri, text_edits)) in changes.iter().enumerate() {
        let text_edits = text_edits.to_vec();
        self
          .apply_text_edits(uri, None, text_edits, offset_encoding)
          .map_err(|kind| ApplyEditError { kind, failed_change_idx: i })?;
      }
    }

    Ok(())
  }

  /// Returns all supported diagnostics for the document
  /// filtered by `filter` which is invocated with the raw `lsp::Diagnostic` and the language server id it came from
  pub fn doc_diagnostics_with_filter<'a>(
    language_servers: &'a helix_lsp::Registry,
    diagnostics: &'a BTreeMap<PathBuf, Vec<(lsp::Diagnostic, usize)>>,

    document: &Document,
    filter: impl Fn(&lsp::Diagnostic, usize) -> bool + 'a,
  ) -> impl Iterator<Item = helix_core::Diagnostic> + 'a {
    let text = document.text().clone();
    let language_config = document.language.clone();
    document
      .path()
      .and_then(|path| diagnostics.get(path))
      .map(|diags| {
        diags.iter().filter_map(move |(diagnostic, lsp_id)| {
          let ls = language_servers.get_by_id(*lsp_id)?;
          language_config
            .as_ref()
            .and_then(|c| {
              c.language_servers.iter().find(|features| {
                features.name == ls.name()
                  && features.has_feature(LanguageServerFeature::Diagnostics)
              })
            })
            .and_then(|_| {
              if filter(diagnostic, *lsp_id) {
                Document::lsp_diagnostic_to_diagnostic(
                  &text,
                  language_config.as_deref(),
                  diagnostic,
                  *lsp_id,
                  ls.offset_encoding(),
                )
              } else {
                None
              }
            })
        })
      })
      .into_iter()
      .flatten()
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

fn try_restore_indent(doc: &mut Document, view: &mut View) {
  use helix_core::{
    chars::char_is_whitespace, line_ending::line_end_char_index, Operation,
    Transaction,
  };

  fn inserted_a_new_blank_line(
    changes: &[Operation],
    pos: usize,
    line_end_pos: usize,
  ) -> bool {
    if let [Operation::Retain(move_pos), Operation::Insert(ref inserted_str), Operation::Retain(_)] =
      changes
    {
      move_pos + inserted_str.len() == pos
        && inserted_str.starts_with('\n')
        && inserted_str.chars().skip(1).all(char_is_whitespace)
        && pos == line_end_pos // ensure no characters exists after current position
    } else {
      false
    }
  }

  let doc_changes = doc.changes().changes();
  let text = doc.text().slice(..);
  let range = doc.selection(view.id).primary();
  let pos = range.cursor(text);
  let line_end_pos = line_end_char_index(&text, range.cursor_line(text));

  if inserted_a_new_blank_line(doc_changes, pos, line_end_pos) {
    // Removes tailing whitespaces.
    let transaction = Transaction::change_by_selection(
      doc.text(),
      doc.selection(view.id),
      |range| {
        let line_start_pos = text.line_to_char(range.cursor_line(text));
        (line_start_pos, pos, None)
      },
    );
    doc.apply(&transaction, view.id);
  }
}
