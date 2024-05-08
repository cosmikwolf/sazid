use crate::{
  commands::ChatMessageItem,
  compositor::{self, Component, Compositor, Context, ContextFocus, Event, EventResult}, filter_picker_entry,
  job::Callback,
  movement::min_width_1,
  ui::{
    document::{render_document, LineDecoration, LinePos, TextRenderer},
    EditorView,
  },
  widgets::table::{MessageCell, MessageType, Row, Table, TableState},
};

use arc_swap::ArcSwap;
use futures_util::{future::BoxFuture, FutureExt};
use nucleo::{Config, Nucleo, Utf32String};

use tui::{
  buffer::Buffer as Surface,
  layout::Constraint,
  widgets::{Block, Borders},
};

use tui::widgets::Widget;

use std::{
  collections::HashMap,
  io::Read,
  path::PathBuf,
  sync::{
    atomic::{self, AtomicBool},
    Arc,
  },
};

use helix_core::{
  char_idx_at_visual_offset,
  fuzzy::MATCHER,
  movement::Direction,
  syntax::{self, Highlight, HighlightEvent},
  text_annotations::TextAnnotations,
  Position, Rope, RopeSlice, Selection, Syntax,
};

use helix_view::{
  document::Mode,
  editor::{Action, CursorShapeConfig},
  graphics::{CursorKind, Margin, Modifier, Rect, UnderlineStyle},
  input::{MouseButton, MouseEventKind},
  theme::{Color, Style},
  view::ViewPosition,
  Document, DocumentId, Editor, Theme,
};

pub const ID: &str = "session";
use super::{markdownmenu::MarkdownItem, overlay::Overlay, Picker};

pub const MIN_AREA_WIDTH_FOR_PREVIEW: u16 = 72;
/// Biggest file size to preview in bytes
pub const MAX_FILE_SIZE_FOR_PREVIEW: u64 = 10 * 1024 * 1024;

#[derive(PartialEq, Eq, Hash)]
pub enum PathOrId {
  Id(DocumentId),
  Path(PathBuf),
}

impl PathOrId {
  fn get_canonicalized(self) -> Self {
    use PathOrId::*;
    match self {
      Path(path) => Path(helix_stdx::path::canonicalize(path)),
      Id(id) => Id(id),
    }
  }
}

impl From<PathBuf> for PathOrId {
  fn from(v: PathBuf) -> Self {
    Self::Path(v)
  }
}

impl From<DocumentId> for PathOrId {
  fn from(v: DocumentId) -> Self {
    Self::Id(v)
  }
}

type FileCallback<T> = Box<dyn Fn(&Editor, &T) -> Option<FileLocation>>;

/// File path and range of lines (used to align and highlight lines)
pub type FileLocation = (PathOrId, Option<(usize, usize)>);

pub enum CachedPreview {
  Document(Box<Document>),
  Binary,
  LargeFile,
  NotFound,
}

// We don't store this enum in the cache so as to avoid lifetime constraints
// from borrowing a document already opened in the editor.
pub enum Preview<'session, 'editor> {
  Cached(&'session CachedPreview),
  EditorDocument(&'editor Document),
}

impl Preview<'_, '_> {
  fn document(&self) -> Option<&Document> {
    match self {
      Preview::EditorDocument(doc) => Some(doc),
      Preview::Cached(CachedPreview::Document(doc)) => Some(doc),
      _ => None,
    }
  }

  /// Alternate text to show for the preview.
  fn placeholder(&self) -> &str {
    match *self {
      Self::EditorDocument(_) => "<Invalid file location>",
      Self::Cached(preview) => match preview {
        CachedPreview::Document(_) => "<Invalid file location>",
        CachedPreview::Binary => "<Binary file>",
        CachedPreview::LargeFile => "<File too large to preview>",
        CachedPreview::NotFound => "<File not found>",
      },
    }
  }
}

pub fn item_to_nucleo<T: MarkdownItem>(item: T, editor_data: &T::Data) -> Option<(T, Utf32String)> {
  let text: String = item.format(editor_data, None).into();
  Some((item, text.into()))
}

pub struct Injector<T: MarkdownItem> {
  dst: nucleo::Injector<T>,
  editor_data: Arc<T::Data>,
  shutown: Arc<AtomicBool>,
}

impl<T: MarkdownItem> Clone for Injector<T> {
  fn clone(&self) -> Self {
    Injector {
      dst: self.dst.clone(),
      editor_data: self.editor_data.clone(),
      shutown: self.shutown.clone(),
    }
  }
}

pub struct InjectorShutdown;

impl<T: MarkdownItem> Injector<T> {
  pub fn push(&self, item: T) -> Result<(), InjectorShutdown> {
    if self.shutown.load(atomic::Ordering::Relaxed) {
      return Err(InjectorShutdown);
    }

    if let Some((item, matcher_text)) = item_to_nucleo(item, &self.editor_data) {
      self.dst.push(item, |dst| dst[0] = matcher_text);
    }
    Ok(())
  }
}

pub struct SessionView<T: MarkdownItem> {
  editor_data: Arc<T::Data>,
  shutdown: Arc<AtomicBool>,
  matcher: Nucleo<T>,
  pub messages: Vec<ChatMessageItem>,

  /// Current height of the completions box
  completion_height: u16,
  terminal_focused: bool,
  session_is_focused: bool,
  selected_option: u32,
  syn_loader: Arc<ArcSwap<syntax::Loader>>,
  // textbox: ui::textbox::Textbox,
  pub input: EditorView,
  pub input_height: u16,
  input_hidden: bool,
  pub state: TableState,
  table_column_spacing: u16,
  pub table_row_spacing: u16,
  /// Whether to show the preview panel (default true)
  show_preview: bool,
  /// Constraints for tabular formatting
  widths: Vec<Constraint>,
  line_char_counts: Vec<usize>,
  pub chat_viewport: Rect,
  callback_fn: SessionCallback<T>,
  pub selection: Selection,
  pub truncate_start: bool,
  /// Caches paths to documents
  preview_cache: HashMap<PathBuf, CachedPreview>,
  read_buffer: Vec<u8>,
  /// Given an item in the session, return the file path and line number to display.
  file_fn: Option<FileCallback<T>>,
  messages_plaintext: Rope,
  updating_system_prompt: bool,
}

impl<T: MarkdownItem + 'static> SessionView<T> {
  pub fn stream(editor_data: T::Data) -> (Nucleo<T>, Injector<T>) {
    let matcher = Nucleo::new(Config::DEFAULT, Arc::new(helix_event::request_redraw), None, 1);
    let streamer = Injector {
      dst: matcher.injector(),
      editor_data: Arc::new(editor_data),
      shutown: Arc::new(AtomicBool::new(false)),
    };
    (matcher, streamer)
  }

  pub fn new(
    options: Vec<T>,
    theme: Option<Theme>,
    editor_data: T::Data,
    syn_loader: Arc<ArcSwap<syntax::Loader>>,
    callback_fn: impl Fn(&mut Context, &T, Action) + 'static,
  ) -> Self {
    let matcher = Nucleo::new(Config::DEFAULT, Arc::new(helix_event::request_redraw), None, 1);
    let injector = matcher.injector();
    for item in options {
      if let Some((item, matcher_text)) = item_to_nucleo(item, &editor_data) {
        injector.push(item, |dst| dst[0] = matcher_text);
      }
    }
    Self::with(
      matcher,
      theme,
      Arc::new(editor_data),
      Arc::new(AtomicBool::new(false)),
      syn_loader,
      callback_fn,
    )
  }

  pub fn with_stream(
    matcher: Nucleo<T>,
    theme: Option<Theme>,
    injector: Injector<T>,
    syn_loader: Arc<ArcSwap<syntax::Loader>>,
    callback_fn: impl Fn(&mut Context, &T, Action) + 'static,
  ) -> Self {
    Self::with(matcher, theme, injector.editor_data, injector.shutown, syn_loader, callback_fn)
  }

  fn with(
    matcher: Nucleo<T>,
    _theme: Option<Theme>,
    editor_data: Arc<T::Data>,
    shutdown: Arc<AtomicBool>,
    syn_loader: Arc<ArcSwap<syntax::Loader>>,
    callback_fn: impl Fn(&mut Context, &T, Action) + 'static,
  ) -> Self {
    let input_height = 10;

    let input = EditorView::new(crate::keymap::minimal_keymap());
    let tablestate = TableState {
      scroll_offset: input_height + 5,
      vertical_scroll: 0,
      sticky_scroll: true,
      scroll_max: 0,
      selected: None,
      row_heights: Vec::new(),
      viewport_height: 0,
      select_range: None,
      cursor_position: None,
      cursor_style: None,
    };

    Self {
      messages: Vec::new(),
      matcher,
      editor_data,
      shutdown,
      session_is_focused: false,
      terminal_focused: true,
      selected_option: 0,
      line_char_counts: Vec::new(),
      state: tablestate,
      input,
      syn_loader,
      chat_viewport: Rect::default(),
      input_height,
      input_hidden: false,
      table_column_spacing: 1,
      table_row_spacing: 1,
      truncate_start: true,
      show_preview: true,
      callback_fn: Box::new(callback_fn),
      completion_height: 0,
      widths: Vec::new(),
      preview_cache: HashMap::new(),
      read_buffer: Vec::with_capacity(1024),
      file_fn: None,
      selection: Selection::point(0),
      messages_plaintext: Rope::new(),
      updating_system_prompt: false,
    }
  }

  pub fn get_messages_plaintext(&self) -> RopeSlice {
    self.messages_plaintext.slice(..)
  }

  pub fn update_messages_plaintext(&mut self) -> Rope {
    let newlines_per_messages = 1 + self.table_row_spacing as usize;
    if self.messages_plaintext.len_chars()
      != self
        .messages
        .iter()
        .map(|m| m.plain_text.len_chars() + newlines_per_messages)
        .sum::<usize>()
    {
      // log::warn!(
      //   "updating messages plaintext {}  {} {} ",
      //   self.messages_plaintext.len_chars(),
      //   self.messages.iter().map(|m| m.plain_text.len_chars()).sum::<usize>(),
      //   self
      //     .messages
      //     .iter()
      //     .map(|m| m.plain_text.len_chars() + newlines_per_messages)
      //     .sum::<usize>()
      // );
      self.messages_plaintext = Rope::new();
      self.messages.iter_mut().for_each(|message| {
        message.start_idx = self.messages_plaintext.len_chars();
        self.messages_plaintext.append(message.plain_text.clone());
        self.messages_plaintext.append(Rope::from("\n".repeat(newlines_per_messages)));
      });
    };
    self.messages_plaintext.clone()
  }

  pub fn upsert_message(&mut self, message: ChatMessageItem) {
    if let Some(existing_message) =
      self.messages.iter_mut().find(|m| m.id.is_some() && m.id == message.id)
    {
      existing_message.update_message(message.chat_message);
      existing_message.cache_wrapped_plain_text(self.chat_viewport.width, &self.syn_loader);
      self.update_messages_plaintext();
    } else {
      self.messages.push(message);
      self
        .messages
        .iter_mut()
        .last()
        .unwrap()
        .cache_wrapped_plain_text(self.chat_viewport.width, &self.syn_loader);
      self.update_messages_plaintext();
    }
  }

  pub fn reload_messages(&mut self, messages: Vec<ChatMessageItem>) {
    self.messages = messages;
    self.messages.iter_mut().for_each(|message| {
      message.cache_wrapped_plain_text(self.chat_viewport.width, &self.syn_loader);
    });
    self.update_messages_plaintext();
    self.state.scroll_top();
  }

  pub fn set_terminal_focused(&mut self, terminal_focused: bool) {
    self.terminal_focused = terminal_focused
  }

  pub fn injector(&self) -> Injector<T> {
    Injector {
      dst: self.matcher.injector(),
      editor_data: self.editor_data.clone(),
      shutown: self.shutdown.clone(),
    }
  }

  pub fn truncate_start(mut self, truncate_start: bool) -> Self {
    self.truncate_start = truncate_start;
    self
  }

  pub fn with_preview(
    mut self,
    preview_fn: impl Fn(&Editor, &T) -> Option<FileLocation> + 'static,
  ) -> Self {
    self.file_fn = Some(Box::new(preview_fn));
    // assumption: if we have a preview we are matching paths... If this is ever
    // not true this could be a separate builder function
    self.matcher.update_config(Config::DEFAULT.match_paths());
    self
  }

  pub fn set_options(&mut self, new_options: Vec<T>) {
    self.matcher.restart(false);
    let injector = self.matcher.injector();
    for item in new_options {
      if let Some((item, matcher_text)) = item_to_nucleo(item, &self.editor_data) {
        injector.push(item, |dst| dst[0] = matcher_text);
      }
    }
  }

  /// Move the cursor by a number of lines, either down (`Forward`) or up (`Backward`)
  pub fn move_by(&mut self, amount: u16, direction: Direction) {
    let len = self.matcher.snapshot().matched_item_count();
    if len == 0 {
      // No results, can't move.
      return;
    }

    match direction {
      Direction::Forward => {
        self.state.selected = match self.state.selected {
          Some(selected) => {
            Some(selected.saturating_add(amount as usize).clamp(0, len as usize - 1))
          },
          None => Some(0_usize),
        };
        self.state.scroll_to_selection()
      },
      Direction::Backward => {
        self.state.selected = match self.state.selected {
          Some(selected) => Some(selected.saturating_sub(amount as usize)),
          None => Some(0_usize),
        };
        self.state.scroll_to_selection()
      },
    }
  }

  pub fn scroll_up(&mut self) {
    // self.move_by(1, Direction::Backward);
    self.state.scroll_by(1, Direction::Forward);
  }

  pub fn scroll_down(&mut self) {
    // self.move_by(1, Direction::Backward);
    self.state.scroll_by(1, Direction::Backward);
  }
  /// Move the cursor down by exactly one page. After the last page comes the first page.
  pub fn page_up(&mut self) {
    self.move_by(self.completion_height, Direction::Backward);
  }

  /// Move the cursor up by exactly one page. After the first page comes the last page.
  pub fn page_down(&mut self) {
    self.move_by(self.completion_height, Direction::Forward);
  }

  /// Move the cursor to the first entry
  pub fn to_start(&mut self) {
    self.selected_option = 0;
  }

  /// Move the cursor to the last entry
  pub fn to_end(&mut self) {
    self.selected_option = self.matcher.snapshot().matched_item_count().saturating_sub(1);
  }

  pub fn selection(&self) -> Option<&T> {
    self.matcher.snapshot().get_matched_item(self.selected_option).map(|item| item.data)
  }

  pub fn toggle_preview(&mut self) {
    self.show_preview = !self.show_preview;
  }

  fn prompt_handle_event(&mut self, _event: &Event, _cx: &mut Context) -> EventResult {
    // if let EventResult::Consumed(_) = self.textbox.handle_event(event, cx) {
    //   let pattern = self.textbox.line();
    //   // TODO: better track how the pattern has changed
    //   if pattern != &self.previous_pattern {
    //     self.matcher.pattern.reparse(
    //       0,
    //       pattern,
    //       CaseMatching::Smart,
    //       pattern.starts_with(&self.previous_pattern),
    //     );
    //     self.previous_pattern = pattern.clone();
    //   }
    // }
    EventResult::Consumed(None)
  }

  fn current_file(&self, editor: &Editor) -> Option<FileLocation> {
    self
      .selection()
      .and_then(|current| (self.file_fn.as_ref()?)(editor, current))
      .map(|(path_or_id, line)| (path_or_id.get_canonicalized(), line))
  }

  /// Get (cached) preview for a given path. If a document corresponding
  /// to the path is already open in the editor, it is used instead.
  fn get_preview<'session, 'editor>(
    &'session mut self,
    path_or_id: PathOrId,
    editor: &'editor Editor,
  ) -> Preview<'session, 'editor> {
    match path_or_id {
      PathOrId::Path(path) => {
        let path = &path;
        if let Some(doc) = editor.document_by_path(path) {
          return Preview::EditorDocument(doc);
        }

        if self.preview_cache.contains_key(path) {
          return Preview::Cached(&self.preview_cache[path]);
        }

        let data = std::fs::File::open(path).and_then(|file| {
          let metadata = file.metadata()?;
          // Read up to 1kb to detect the content type
          let n = file.take(1024).read_to_end(&mut self.read_buffer)?;
          let content_type = content_inspector::inspect(&self.read_buffer[..n]);
          self.read_buffer.clear();
          Ok((metadata, content_type))
        });
        let preview = data
          .map(|(metadata, content_type)| match (metadata.len(), content_type) {
            (_, content_inspector::ContentType::BINARY) => CachedPreview::Binary,
            (size, _) if size > MAX_FILE_SIZE_FOR_PREVIEW => CachedPreview::LargeFile,
            _ => Document::open(path, None, None, editor.config.clone())
              .map(|doc| CachedPreview::Document(Box::new(doc)))
              .unwrap_or(CachedPreview::NotFound),
          })
          .unwrap_or(CachedPreview::NotFound);
        self.preview_cache.insert(path.to_owned(), preview);
        Preview::Cached(&self.preview_cache[path])
      },
      PathOrId::Id(id) => {
        let doc = editor.documents.get(&id).unwrap();
        Preview::EditorDocument(doc)
      },
    }
  }

  fn handle_idle_timeout(&mut self, cx: &mut Context) -> EventResult {
    let Some((current_file, _)) = self.current_file(cx.editor) else {
      return EventResult::Consumed(None);
    };

    // Try to find a document in the cache
    let doc = match &current_file {
      PathOrId::Id(doc_id) => doc_mut!(cx.editor, doc_id),
      PathOrId::Path(path) => match self.preview_cache.get_mut(path) {
        Some(CachedPreview::Document(ref mut doc)) => doc,
        _ => return EventResult::Consumed(None),
      },
    };

    let mut callback: Option<compositor::Callback> = None;

    // Then attempt to highlight it if it has no language set
    if doc.language_config().is_none() {
      if let Some(language_config) = doc.detect_language_config(&cx.editor.syn_loader.load()) {
        doc.language = Some(language_config.clone());
        let text = doc.text().clone();
        let loader = cx.editor.syn_loader.clone();
        let job = tokio::task::spawn_blocking(move || {
          let syntax = language_config
            .highlight_config(&loader.load().scopes())
            .and_then(|highlight_config| Syntax::new(text.slice(..), highlight_config, loader));
          let callback = move |editor: &mut Editor, compositor: &mut Compositor| {
            let Some(syntax) = syntax else {
              log::info!("highlighting session item failed");
              return;
            };
            let session = match compositor.find::<Overlay<Self>>() {
              Some(Overlay { content, .. }) => Some(content),
              None => compositor
                .find::<Overlay<DynamicSession<T>>>()
                .map(|overlay| &mut overlay.content.file_session),
            };
            let Some(session) = session else {
              log::info!("session closed before syntax highlighting finished");
              return;
            };
            // Try to find a document in the cache
            let doc = match current_file {
              PathOrId::Id(doc_id) => doc_mut!(editor, &doc_id),
              PathOrId::Path(path) => match session.preview_cache.get_mut(&path) {
                Some(CachedPreview::Document(ref mut doc)) => {
                  let diagnostics =
                    Editor::doc_diagnostics(&editor.language_servers, &editor.diagnostics, doc);
                  doc.replace_diagnostics(diagnostics, &[], None);
                  doc
                },
                _ => return,
              },
            };
            doc.syntax = Some(syntax);
          };
          Callback::EditorCompositor(Box::new(callback))
        });
        let tmp: compositor::Callback = Box::new(move |_, ctx| {
          ctx.jobs.callback(job.map(|res| res.map_err(anyhow::Error::from)))
        });
        callback = Some(Box::new(tmp))
      }
    }

    // QUESTION: do we want to compute inlay hints in sessions too ? Probably not for now
    // but it could be interesting in the future

    EventResult::Consumed(callback)
  }

  fn render_session(
    &mut self,
    area: Rect,
    surface: &mut Surface,
    cx: &mut Context,
    _overlay_highlight_iter: impl Iterator<Item = HighlightEvent>,
  ) {
    // -- make space for the input bar:
    let input_on_top = false;
    // define input area
    let area = if self.input_hidden || input_on_top {
      area.clip_top(self.input_height)
    } else {
      area.clip_bottom(self.input_height + 1)
    };

    let status = self.matcher.tick(10);
    let snapshot = self.matcher.snapshot();
    if status.changed {
      self.selected_option =
        self.selected_option.min(snapshot.matched_item_count().saturating_sub(1))
    }

    let text_style = cx.editor.theme.get("ui.text");
    let _cursor_style = cx.editor.theme.get("ui.cursor");
    let selected = cx.editor.theme.get("ui.selection");
    let _highlight_style = cx.editor.theme.get("special").add_modifier(Modifier::BOLD);

    // -- Render the frame:
    // clear area
    let background = cx.editor.theme.get("ui.background");
    surface.clear_with(area, background);
    let block = Block::default().borders(Borders::ALL);

    // calculate the inner area inside the box
    let table_area = block.inner(area);

    block.render(area, surface);

    // -- upper right hand corner readout
    let count = format!(
      "{}{}/{}",
      if status.running { "(running) " } else { "" },
      snapshot.matched_item_count(),
      snapshot.item_count(),
    );

    surface.set_stringn(
      (area.x + area.width).saturating_sub(count.len() as u16 + 1),
      area.y,
      &count,
      (count.len()).min(area.width as usize),
      text_style,
    );

    // -- Render the contents:
    let mut matcher = MATCHER.lock();
    matcher.config = Config::DEFAULT;
    if self.file_fn.is_some() {
      matcher.config.set_match_paths()
    }

    if let (Some(position), _cursor) = self.cursor(self.chat_viewport, cx.editor) {
      self.state.cursor_position = Some(position);
    };

    let primary_range = self.selection.primary();

    let highlight_range = if primary_range.head < primary_range.anchor {
      std::ops::Range { start: primary_range.head, end: primary_range.anchor }
    } else {
      std::ops::Range { start: primary_range.anchor, end: primary_range.head }
    };
    let highlight_style = selected;

    // precalculate column areas so plain text messages can be cached
    self.widths = vec![Constraint::Length(5), Constraint::Percentage(25)];
    let highlight_symbol = " > ".to_string();
    let column_areas = Table::calculate_column_areas(
      table_area,
      self.state.selected.is_some(),
      self.widths.as_slice(),
      self.table_column_spacing,
      Some(highlight_symbol),
    );
    self.chat_viewport = column_areas[1];

    // self.messages.iter_mut().for_each(|message| {
    // message.update_wrapped_plain_text_if_necessary(self.chat_viewport.width, &self.syn_loader)
    // });

    Table::new(
      self
        .messages
        .iter()
        .enumerate()
        .map(|(msg_idx, message)| {
          let message_cell = MessageCell::new(MessageType::Chat(message))
            // .with_style(style)
            .with_wrap_trim(false)
            .with_highlight(highlight_style, highlight_range.clone())
            .with_block(Block::default())
            .with_char_index(message.start_idx);

          let msg_idx = msg_idx.to_string();
          let index_cell = MessageCell::new(MessageType::Text(msg_idx))
            .centered()
            .with_block(Block::default().borders(Borders::RIGHT));

          Row::new(vec![index_cell, message_cell]).height(message.plain_text.len_lines() as u16)
        })
        .collect::<Vec<Row>>(),
    )
    .style(text_style)
    .highlight_style(selected)
    .highlight_symbol(" > ")
    .column_spacing(self.table_column_spacing)
    .row_spacing(self.table_row_spacing)
    .widths(&self.widths)
    .render_table(
      table_area,
      surface,
      &mut self.state,
      self.truncate_start,
      &cx.editor.theme,
      &cx.editor.syn_loader,
    );
  }

  fn viewport_byte_range(
    text: helix_core::RopeSlice,
    row: usize,
    height: u16,
  ) -> std::ops::Range<usize> {
    // Calculate viewport byte ranges:
    // Saturating subs to make it inclusive zero indexing.
    let last_line = text.len_lines().saturating_sub(1);
    let last_visible_line = (row + height as usize).saturating_sub(1).min(last_line);
    let start = text.line_to_byte(row.min(last_line));
    let end = text.line_to_byte(last_visible_line + 1);

    start..end
  }

  pub fn empty_highlight_iter(
    text: helix_core::RopeSlice<'_>,
    anchor: usize,
    height: u16,
  ) -> Box<dyn Iterator<Item = HighlightEvent>> {
    let row = text.char_to_line(anchor.min(text.len_chars()));

    // Calculate viewport byte ranges:
    // Saturating subs to make it inclusive zero indexing.
    let range = Self::viewport_byte_range(text, row, height);
    Box::new(
      [HighlightEvent::Source {
        start: text.byte_to_char(range.start),
        end: text.byte_to_char(range.end),
      }]
      .into_iter(),
    )
  }

  fn get_selection_highlights(
    &mut self,
    area: Rect,
    _surface: &mut Surface,
    cx: &mut Context,
  ) -> Box<dyn Iterator<Item = HighlightEvent>> {
    let overlay_highlights_spans = if self.session_is_focused {
      self.session_selection_highlights(
        cx.editor.mode(),
        &cx.editor.theme,
        &cx.editor.config().cursor_shape,
        self.terminal_focused,
      )
    } else {
      vec![]
    };

    let text = self.get_messages_plaintext();
    

    (Box::new(helix_core::syntax::merge(
      Self::empty_highlight_iter(text, 7, area.height),
      overlay_highlights_spans,
    ))) as _

    // let mut overlay_styles = StyleIter {
    //   text_style: Style::default(),
    //   active_highlights: Vec::with_capacity(64),
    //   highlight_iter: overlay_highlights,
    //   theme: &cx.editor.theme,
    // };

    // if let (Some(pos), kind) = self.cursor(area, cx.editor) {
    //   let x = (pos.row as u16).clamp(area.left(), area.right());
    //   let y = (pos.col as u16).clamp(area.top(), area.bottom());
    //   let cursor_area = Rect::new(x, y, 1, 1);
    //   surface.set_style(cursor_area, cx.editor.theme.get("ui.cursor"));
    // }
    // let cursor = overlay_styles.next();

    // while let Some(overlay) = overlay_styles.next() {
    //   let pos = self.translate_char_index_to_pos(self.chat_viewport, overlay.1);
    //   log::warn!("pos: {:?}\toverlay:{:?}", pos, overlay.1);
    //   let area = Rect::new(pos.col as u16, pos.row as u16, 1, 1);
    //   let cell = surface.get(pos.col as u16, pos.row as u16);
    //
    //   let style = match cell {
    //     Some(cell) => cell.style().patch(overlay.0),
    //     None => overlay.0,
    //   };
    //   surface.set_style(area, style);
    // }
    //
    // while let (Some(overlay_start), Some(overlay_end)) =
    //   (overlay_styles.next(), overlay_styles.next())
    // {
    //   log::warn!(
    //     "overlay_start: {:?}, overlay_end: {:?}",
    //     overlay_start.1,
    //     overlay_end.1
    //   );
    //   let start_pos =
    //     self.translate_char_index_to_pos(self.chat_viewport, overlay_start.1);
    //   let end_pos =
    //     self.translate_char_index_to_pos(self.chat_viewport, overlay_end.1);
    //   let selection_height =
    //     (end_pos.row.saturating_sub(start_pos.row) + 1) as u16;
    //
    //   log::warn!(
    //     "\nstart_pos: {:?}\nend_pos: {:?}\nheight: {}",
    //     start_pos,
    //     end_pos,
    //     selection_height
    //   );
    //   // the first rectangle is from start_pos to end_pos, or the end of the line, whichever is first
    //   // if the selection is at least 2 lines,
    //   // then the second rectangle is from the start of the line to end_pos
    //   // if the selection is more than 2 lines, then the third rectangle is from the start of the line to the end of the line for all lines inbetween
    //   let selection_top_width = if selection_height == 1 {
    //     end_pos.col.saturating_sub(start_pos.col) as u16
    //   } else {
    //     match text.get_line(end_pos.row) {
    //       Some(line) => line.len_chars().saturating_sub(end_pos.col) as u16,
    //       None => 1,
    //     }
    //   };
    //
    //   let selection_top = Rect::new(
    //     start_pos.col as u16,
    //     start_pos.row as u16,
    //     selection_top_width,
    //     1,
    //   );
    //   surface.set_style(selection_top, overlay_end.0);
    //   log::warn!("selection_top: {:?}", selection_top);
    //
    //   if selection_height > 1 {
    //     let selection_end =
    //       Rect::new(0, end_pos.row as u16, end_pos.col as u16, 1);
    //     surface.set_style(selection_end, overlay_start.0);
    //     log::warn!("selection_end: {:?}", selection_end);
    //   }
    //
    //   if selection_height > 2 {
    //     for row in start_pos.row + 1..end_pos.row - 1 {
    //       let selection_body =
    //         Rect::new(0, row as u16, text.line(row).len_chars() as u16, 1);
    //       surface.set_style(selection_body, overlay_start.0);
    //       log::warn!("selection_body: {:?}", selection_body);
    //     }
    //   }
    // }
  }

  /// Get highlight spans for selections in a document view.
  pub fn session_selection_highlights(
    &self,
    mode: Mode,
    theme: &Theme,
    cursor_shape_config: &CursorShapeConfig,
    is_terminal_focused: bool,
  ) -> Vec<(usize, std::ops::Range<usize>)> {
    let text = self.get_messages_plaintext();
    let text = text.slice(..);
    let selection = &self.selection;
    let primary_idx = selection.primary_index();

    let cursorkind = cursor_shape_config.from_mode(mode);
    let cursor_is_block = cursorkind == CursorKind::Block;

    let selection_scope = theme
      .find_scope_index_exact("ui.selection")
      .expect("could not find `ui.selection` scope in the theme!");
    let primary_selection_scope =
      theme.find_scope_index_exact("ui.selection.primary").unwrap_or(selection_scope);

    let base_cursor_scope = theme.find_scope_index_exact("ui.cursor").unwrap_or(selection_scope);
    let base_primary_cursor_scope =
      theme.find_scope_index("ui.cursor.primary").unwrap_or(base_cursor_scope);

    let cursor_scope = match mode {
      Mode::Insert => theme.find_scope_index_exact("ui.cursor.insert"),
      Mode::Select => theme.find_scope_index_exact("ui.cursor.select"),
      Mode::Normal => theme.find_scope_index_exact("ui.cursor.normal"),
    }
    .unwrap_or(base_cursor_scope);

    let primary_cursor_scope = match mode {
      Mode::Insert => theme.find_scope_index_exact("ui.cursor.primary.insert"),
      Mode::Select => theme.find_scope_index_exact("ui.cursor.primary.select"),
      Mode::Normal => theme.find_scope_index_exact("ui.cursor.primary.normal"),
    }
    .unwrap_or(base_primary_cursor_scope);

    // log::info!("selection: {:#?}", selection);

    let mut spans: Vec<(usize, std::ops::Range<usize>)> = Vec::new();
    for (i, range) in selection.iter().enumerate() {
      let selection_is_primary = i == primary_idx;
      let (cursor_scope, _selection_scope) = if selection_is_primary {
        (primary_cursor_scope, primary_selection_scope)
      } else {
        (cursor_scope, selection_scope)
      };

      // Special-case: cursor at end of the rope.
      if range.head == range.anchor && range.head == text.len_chars() {
        if !selection_is_primary || (cursor_is_block && is_terminal_focused) {
          // Bar and underline cursors are drawn by the terminal
          // BUG: If the editor area loses focus while having a bar or
          // underline cursor (eg. when a regex prompt has focus) then
          // the primary cursor will be invisible. This doesn't happen
          // with block cursors since we manually draw *all* cursors.
          spans.push((cursor_scope, range.head..range.head + 1));
        }
        continue;
      }

      let range = min_width_1(range);
      // log::info!("range: {:?}", range);
      if range.head > range.anchor {
        // Standard case.
        /*
        // let cursor_start = prev_grapheme_boundary(text, range.head);
        let cursor_start = range.head;
        // non block cursors look like they exclude the cursor
        let selection_end =
          if selection_is_primary && !cursor_is_block && mode != Mode::Insert {
            range.head
          } else {
            cursor_start
          };
        spans.push((selection_scope, range.anchor..selection_end));
        // add block cursors
        // skip primary cursor if terminal is unfocused - crossterm cursor is used in that case
        if !selection_is_primary || (cursor_is_block && is_terminal_focused) {
          spans.push((cursor_scope, cursor_start..range.head));
        }
        */
        // log::info!("standard case: head {} anchor {}", range.head, range.anchor);

        spans.push((cursor_scope, range.head..range.head));
      } else {
        // Reverse case.
        /*
        // let cursor_end = next_grapheme_boundary(text, range.head);
        let cursor_end = range.head;
        // add block cursors
        // skip primary cursor if terminal is unfocused - crossterm cursor is used in that case
        if !selection_is_primary || (cursor_is_block && is_terminal_focused) {
          spans.push((cursor_scope, range.head..cursor_end));
        }
        // non block cursors look like they exclude the cursor
        let selection_start = if selection_is_primary
          && !cursor_is_block
          && !(mode == Mode::Insert && cursor_end == range.anchor)
        {
          range.head
        } else {
          cursor_end
        };
        spans.push((selection_scope, selection_start..range.anchor));
          */
        log::info!("reverse case: cursor head {} anchor {}", range.head, range.anchor);
        spans.push((cursor_scope, range.head..range.anchor));
      }
    }

    spans
  }
  fn render_preview(&mut self, area: Rect, surface: &mut Surface, cx: &mut Context) {
    // -- Render the frame:
    // clear area
    let background = cx.editor.theme.get("ui.background");
    let text = cx.editor.theme.get("ui.text");
    surface.clear_with(area, background);

    // don't like this but the lifetime sucks
    let block = Block::default().borders(Borders::ALL);

    // calculate the inner area inside the box
    let inner = block.inner(area);
    // 1 column gap on either side
    let margin = Margin::horizontal(1);
    let inner = inner.inner(&margin);
    block.render(area, surface);

    if let Some((path, range)) = self.current_file(cx.editor) {
      let preview = self.get_preview(path, cx.editor);
      let doc = match preview.document() {
        Some(doc)
          if range.map_or(true, |(start, end)| start <= end && end <= doc.text().len_lines()) =>
        {
          doc
        },
        _ => {
          let alt_text = preview.placeholder();
          let x = inner.x + inner.width.saturating_sub(alt_text.len() as u16) / 2;
          let y = inner.y + inner.height / 2;
          surface.set_stringn(x, y, alt_text, inner.width as usize, text);
          return;
        },
      };

      let mut offset = ViewPosition::default();
      if let Some((start_line, end_line)) = range {
        let height = end_line - start_line;
        let text = doc.text().slice(..);
        let start = text.line_to_char(start_line);
        let middle = text.line_to_char(start_line + height / 2);
        if height < inner.height as usize {
          let text_fmt = doc.text_format(inner.width, None);
          let annotations = TextAnnotations::default();
          (offset.anchor, offset.vertical_offset) = char_idx_at_visual_offset(
            text,
            middle,
            // align to middle
            -(inner.height as isize / 2),
            0,
            &text_fmt,
            &annotations,
          );
          if start < offset.anchor {
            offset.anchor = start;
            offset.vertical_offset = 0;
          }
        } else {
          offset.anchor = start;
        }
      }

      let syntax_highlights =
        EditorView::doc_syntax_highlights(doc, offset.anchor, area.height, &cx.editor.theme);

      let mut overlay_highlights =
        EditorView::empty_highlight_iter(doc, offset.anchor, area.height);
      for spans in EditorView::doc_diagnostics_highlights(doc, &cx.editor.theme) {
        if spans.is_empty() {
          continue;
        }
        overlay_highlights = Box::new(helix_core::syntax::merge(overlay_highlights, spans));
      }
      let mut decorations: Vec<Box<dyn LineDecoration>> = Vec::new();

      if let Some((start, end)) = range {
        let style = cx
          .editor
          .theme
          .try_get("ui.highlight")
          .unwrap_or_else(|| cx.editor.theme.get("ui.selection"));
        let draw_highlight = move |renderer: &mut TextRenderer, pos: LinePos| {
          if (start..=end).contains(&pos.doc_line) {
            let area = Rect::new(
              renderer.viewport.x,
              renderer.viewport.y + pos.visual_line,
              renderer.viewport.width,
              1,
            );
            renderer.surface.set_style(area, style)
          }
        };
        decorations.push(Box::new(draw_highlight))
      }

      render_document(
        surface,
        inner,
        doc,
        offset,
        // TODO: compute text annotations asynchronously here (like inlay hints)
        &TextAnnotations::default(),
        syntax_highlights,
        overlay_highlights,
        &cx.editor.theme,
        &mut decorations,
        &mut [],
      );
    }
  }
}

/// A wrapper around a HighlightIterator
/// that merges the layered highlights to create the final text style
/// and yields the active text style and the char_idx where the active
/// style will have to be recomputed.
pub struct StyleIter<'a, H: Iterator<Item = HighlightEvent>> {
  text_style: Style,
  active_highlights: Vec<Highlight>,
  highlight_iter: H,
  theme: &'a Theme,
}

impl<H: Iterator<Item = HighlightEvent>> Iterator for StyleIter<'_, H> {
  type Item = (Style, usize);
  fn next(&mut self) -> Option<(Style, usize)> {
    while let Some(event) = self.highlight_iter.next() {
      match event {
        HighlightEvent::HighlightStart(highlights) => self.active_highlights.push(highlights),
        HighlightEvent::HighlightEnd => {
          self.active_highlights.pop();
        },
        HighlightEvent::Source { start, end } => {
          if start == end {
            continue;
          }
          let style = self
            .active_highlights
            .iter()
            .fold(self.text_style, |acc, span| acc.patch(self.theme.highlight(span.0)));
          return Some((style, end));
        },
      }
    }
    None
  }
}
impl<T: MarkdownItem + 'static + Send + Sync> Component for SessionView<T> {
  fn render(&mut self, area: Rect, surface: &mut Surface, cx: &mut Context) {
    match cx.focus {
      ContextFocus::SessionView => {
        self.session_is_focused = true;
      },
      ContextFocus::EditorView => {
        self.session_is_focused = false;
      },
    }
    // +---------+ +---------+
    // |prompt   | |preview  |
    // +---------+ |         |
    // |session   | |         |
    // |         | |         |
    // +---------+ +---------+

    let render_preview =
      self.show_preview && self.file_fn.is_some() && area.width > MIN_AREA_WIDTH_FOR_PREVIEW;

    let session_width = if render_preview { area.width / 2 } else { area.width };

    let session_area = area.with_width(session_width);

    let selection_highlights = self.get_selection_highlights(session_area, surface, cx);

    self.render_session(session_area, surface, cx, selection_highlights);

    if let (Some(pos), _kind) = self.cursor(area, cx.editor) {
      // log::debug!("Cursor Position: {:?}", pos);
      let cursor_area = Rect { x: pos.col as u16, y: pos.row as u16, width: 1, height: 1 };
      if cursor_area.intersects(area) {
        surface.set_style(
          cursor_area,
          Style::default()
            .underline_style(UnderlineStyle::Curl)
            .underline_color(Color::Magenta)
            .bg(Color::Blue),
        )
      } else {
        log::error!("CURSOR OUT OF BOUNDS {:?} not within {:?}", cursor_area, area);
      }
    };
    // if render_preview {
    //   let preview_area = area.clip_left(session_width);
    //   self.render_preview(preview_area, surface, cx);
    // }
  }

  fn handle_event(&mut self, event: &Event, ctx: &mut Context) -> EventResult {
    // log::info!("session events--: {:?}", event);

    let event_result = match event {
      Event::IdleTimeout => self.handle_idle_timeout(ctx),
      Event::Resize(..) => {
        self.update_messages_plaintext();
        EventResult::Consumed(None)
      },
      Event::Mouse(event) => {
        match event.kind {
          MouseEventKind::Down(MouseButton::Left) => {
            if let Some(char_idx) = crate::movement::translate_pos_to_char_index(
              &self.get_messages_plaintext(),
              self.chat_viewport,
              self.state.vertical_scroll,
              Position { row: event.row as usize, col: event.column as usize },
            ) {
              self.selection = Selection::point(char_idx);
              helix_event::request_redraw();
              EventResult::Consumed(None)
            } else {
              EventResult::Ignored(None)
            }
            // start select
          },
          MouseEventKind::Up(MouseButton::Left) => {
            EventResult::Ignored(None)
            // stop select
            // log::info!("mouse up event: {:?}", event);
          },
          MouseEventKind::Drag(MouseButton::Left) => {
            if let Some(char_idx) = crate::movement::translate_pos_to_char_index(
              &self.get_messages_plaintext(),
              self.chat_viewport,
              self.state.vertical_scroll,
              Position { row: event.row as usize, col: event.column as usize },
            ) {
              let range = self.selection.primary();
              self.selection = Selection::single(range.anchor, char_idx);
              helix_event::request_redraw();
            }
            // update select
            log::info!("mouse drag event: {:?}", event);
            EventResult::Consumed(None)
          },
          MouseEventKind::ScrollUp => {
            // log::info!("scroll up");
            self.state.scroll_by(1, Direction::Backward);
            helix_event::request_redraw();
            EventResult::Consumed(None)
          },
          MouseEventKind::ScrollDown => {
            // log::info!("scroll down");
            self.state.scroll_by(1, Direction::Forward);
            helix_event::request_redraw();
            EventResult::Consumed(None)
          },
          _ => EventResult::Ignored(None),
        }
      },
      _ => EventResult::Ignored(None),
    };

    // So that idle timeout retriggers
    ctx.editor.reset_idle_timer();
    event_result
  }

  fn cursor(&self, _area: Rect, editor: &Editor) -> (Option<Position>, CursorKind) {
    let text = self.get_messages_plaintext();
    let session_cursor = self.selection.primary().head;

    // let row = text.char_to_line(session_cursor);
    // let col = session_cursor.saturating_sub(text.line(row).len_chars())
    //   + self.chat_viewport.left() as usize;
    //
    // let pos = Position { row: row + self.chat_viewport.top() as usize, col };
    let (_, _, pos) = crate::movement::translate_char_index_to_viewport_pos(
      &text.slice(..),
      self.chat_viewport,
      self.state.vertical_scroll,
      session_cursor,
      false,
    );
    // let mut row = text
    //   .try_char_to_line(session_cursor)
    //   .map_err(|e| format!("cursor out of bounds {}", e))
    //   .unwrap();
    //
    // let char_at_line_start = text
    //   .try_line_to_char(row)
    //   .map_err(|e| format!("cursor out of bounds {}", e))
    //   .unwrap();
    // let mut col = session_cursor.saturating_sub(char_at_line_start);
    // if col > text.line(row).len_chars() {
    //   col = text.line(row).len_chars();
    // }
    // let orig_row = row;
    // log::info!(
    //   "cursor
    //     row: {:#?}
    //     col: {:#?}
    //     session_cursor: {:#?}
    //     char_at_line_start: {:#?}
    //     inner_viewport: {:#?}
    //     text: {:#?}
    //     lines_len: {},
    //     chars_len: {}",
    //   row,
    //   col,
    //   session_cursor,
    //   char_at_line_start,
    //   self.chat_viewport,
    //   text.get_line(orig_row).unwrap_or(Rope::new().slice(..)).to_string(),
    //   text.lines().len(),
    //   text.chars().len(),
    // );

    let cursor_res = if self.session_is_focused && self.terminal_focused {
      (Some(pos), CursorKind::Block)
    } else {
      (None, CursorKind::Hidden)
    };
    // log::info!("cursor: {:?}", cursor_res);
    editor.cursor_cache.set(Some(cursor_res.0));
    cursor_res
  }
  fn required_size(&mut self, (width, height): (u16, u16)) -> Option<(u16, u16)> {
    self.completion_height = height.saturating_sub(4);
    Some((width, height))
  }

  fn id(&self) -> Option<&'static str> {
    Some(ID)
  }
}

impl<T: MarkdownItem> Drop for SessionView<T> {
  fn drop(&mut self) {
    // ensure we cancel any ongoing background threads streaming into the session
    self.shutdown.store(true, atomic::Ordering::Relaxed)
  }
}

type SessionCallback<T> = Box<dyn Fn(&mut Context, &T, Action)>;

/// Returns a new list of options to replace the contents of the session
/// when called with the current session query,
pub type DynQueryCallback<T> =
  Box<dyn Fn(String, &mut Editor) -> BoxFuture<'static, anyhow::Result<Vec<T>>>>;

/// A session that updates its contents via a callback whenever the
/// query string changes. Useful for live grep, workspace symbols, etc.
pub struct DynamicSession<T: MarkdownItem + Send + Sync> {
  file_session: SessionView<T>,
  query_callback: DynQueryCallback<T>,
  query: String,
}

impl<T: MarkdownItem + Send + Sync> DynamicSession<T> {
  pub fn new(file_session: SessionView<T>, query_callback: DynQueryCallback<T>) -> Self {
    Self { file_session, query_callback, query: String::new() }
  }
}

impl<T: MarkdownItem + Send + Sync + 'static> Component for DynamicSession<T> {
  fn render(&mut self, area: Rect, surface: &mut Surface, cx: &mut Context) {
    self.file_session.render(area, surface, cx);
  }

  // fn handle_event(&mut self, event: &Event, cx: &mut Context) -> EventResult {
  //   let event_result = self.file_session.handle_event(event, cx);
  //   let current_query = self.file_session.textbox.line();
  //
  //   if !matches!(event, Event::IdleTimeout) || self.query == *current_query {
  //     return event_result;
  //   }
  //
  //   self.query.clone_from(current_query);
  //
  //   let new_options =
  //     (self.query_callback)(current_query.to_owned(), cx.editor);
  //
  //   cx.jobs.callback(async move {
  //     let new_options = new_options.await?;
  //     let callback =
  //       Callback::EditorCompositor(Box::new(move |editor, compositor| {
  //         // Wrapping of sessions in overlay is done outside the session code,
  //         // so this is fragile and will break if wrapped in some other widget.
  //         let session =
  //           match compositor.find_id::<Overlay<DynamicSession<T>>>(ID) {
  //             Some(overlay) => &mut overlay.content.file_session,
  //             None => return,
  //           };
  //         session.set_options(new_options);
  //         editor.reset_idle_timer();
  //       }));
  //     anyhow::Ok(callback)
  //   });
  //   EventResult::Consumed(None)
  // }

  fn cursor(&self, area: Rect, ctx: &Editor) -> (Option<Position>, CursorKind) {
    self.file_session.cursor(area, ctx)
  }

  fn required_size(&mut self, viewport: (u16, u16)) -> Option<(u16, u16)> {
    self.file_session.required_size(viewport)
  }

  fn id(&self) -> Option<&'static str> {
    Some(ID)
  }
}

pub fn session_picker(root: PathBuf, config: &helix_view::editor::Config) -> Picker<PathBuf> {
  use ignore::{types::TypesBuilder, WalkBuilder};
  use std::time::Instant;

  let now = Instant::now();

  let dedup_symlinks = config.file_picker.deduplicate_links;
  let absolute_root = root.canonicalize().unwrap_or_else(|_| root.clone());

  let mut walk_builder = WalkBuilder::new(&root);
  walk_builder
    .hidden(config.file_picker.hidden)
    .parents(config.file_picker.parents)
    .ignore(config.file_picker.ignore)
    .follow_links(config.file_picker.follow_symlinks)
    .git_ignore(config.file_picker.git_ignore)
    .git_global(config.file_picker.git_global)
    .git_exclude(config.file_picker.git_exclude)
    .sort_by_file_name(|name1, name2| name1.cmp(name2))
    .max_depth(config.file_picker.max_depth)
    .filter_entry(move |entry| filter_picker_entry(entry, &absolute_root, dedup_symlinks));

  walk_builder.add_custom_ignore_filename(helix_loader::config_dir().join("ignore"));
  walk_builder.add_custom_ignore_filename(".helix/ignore");

  // We want to exclude files that the editor can't handle yet
  let mut type_builder = TypesBuilder::new();
  type_builder
    .add("compressed", "*.{zip,gz,bz2,zst,lzo,sz,tgz,tbz2,lz,lz4,lzma,lzo,z,Z,xz,7z,rar,cab}")
    .expect("Invalid type definition");
  type_builder.negate("all");
  let excluded_types = type_builder.build().expect("failed to build excluded_types");
  walk_builder.types(excluded_types);
  let mut files = walk_builder.build().filter_map(|entry| {
    let entry = entry.ok()?;
    if !entry.file_type()?.is_file() {
      return None;
    }
    Some(entry.into_path())
  });
  log::debug!("file_picker init {:?}", Instant::now().duration_since(now));

  let picker = Picker::new(Vec::new(), root, move |cx, path: &PathBuf, _action| {
    if let Err(e) = cx.session.load_session(path) {
      // let err = if let Some(err) = e.source() {
      //   format!("{}", err)
      // } else {
      let err = format!("unable to open \"{}\" {}", path.display(), e);
      // };
      cx.editor.set_error(err);
    }
  })
  .with_preview(|_editor, path| Some((path.clone().into(), None)));
  let injector = picker.injector();
  let timeout = std::time::Instant::now() + std::time::Duration::from_millis(30);

  let mut hit_timeout = false;
  for file in &mut files {
    if injector.push(file).is_err() {
      break;
    }
    if std::time::Instant::now() >= timeout {
      hit_timeout = true;
      break;
    }
  }
  if hit_timeout {
    std::thread::spawn(move || {
      for file in files {
        if injector.push(file).is_err() {
          break;
        }
      }
    });
  }
  picker
}
