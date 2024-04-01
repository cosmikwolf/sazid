use crate::{
  commands::{ChatMessageItem, ChatMessageType},
  compositor::{self, Component, Compositor, Context, Event, EventResult},
  ctrl, filter_picker_entry,
  job::Callback,
  key, shift,
  ui::{
    document::{render_document, LineDecoration, LinePos, TextRenderer},
    EditorView,
  },
  widgets::table::{Cell, Row, Table, TableState},
};

use futures_util::{future::BoxFuture, FutureExt};
use helix_lsp::lsp::Range;
use nucleo::{Config, Nucleo, Utf32String};

use tui::{
  buffer::Buffer as Surface,
  layout::Constraint,
  widgets::{Block, BorderType, Borders},
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
  char_idx_at_visual_offset, fuzzy::MATCHER, movement::Direction,
  text_annotations::TextAnnotations, Position, Syntax,
};

use helix_view::{
  editor::Action,
  graphics::{CursorKind, Margin, Modifier, Rect},
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

pub fn item_to_nucleo<T: MarkdownItem>(
  item: T,
  editor_data: &T::Data,
) -> Option<(T, Utf32String)> {
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

    if let Some((item, matcher_text)) = item_to_nucleo(item, &self.editor_data)
    {
      self.dst.push(item, |dst| dst[0] = matcher_text);
    }
    Ok(())
  }
}

pub struct SessionView<T: MarkdownItem> {
  editor_data: Arc<T::Data>,
  shutdown: Arc<AtomicBool>,
  matcher: Nucleo<T>,
  messages: Vec<ChatMessageItem>,

  /// Current height of the completions box
  completion_height: u16,

  is_focused: bool,
  theme: Option<Theme>,
  selected_option: u32,
  // textbox: ui::textbox::Textbox,
  pub input: EditorView,
  pub input_height: u16,
  input_hidden: bool,
  state: TableState,
  /// Whether to show the preview panel (default true)
  show_preview: bool,
  /// Constraints for tabular formatting
  widths: Vec<Constraint>,

  callback_fn: SessionCallback<T>,

  pub truncate_start: bool,
  /// Caches paths to documents
  preview_cache: HashMap<PathBuf, CachedPreview>,
  read_buffer: Vec<u8>,
  /// Given an item in the session, return the file path and line number to display.
  file_fn: Option<FileCallback<T>>,
}

impl<T: MarkdownItem + 'static> SessionView<T> {
  pub fn stream(editor_data: T::Data) -> (Nucleo<T>, Injector<T>) {
    let matcher = Nucleo::new(
      Config::DEFAULT,
      Arc::new(helix_event::request_redraw),
      None,
      1,
    );
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
    callback_fn: impl Fn(&mut Context, &T, Action) + 'static,
  ) -> Self {
    let matcher = Nucleo::new(
      Config::DEFAULT,
      Arc::new(helix_event::request_redraw),
      None,
      1,
    );
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
      callback_fn,
    )
  }

  pub fn with_stream(
    matcher: Nucleo<T>,
    theme: Option<Theme>,
    injector: Injector<T>,
    callback_fn: impl Fn(&mut Context, &T, Action) + 'static,
  ) -> Self {
    Self::with(
      matcher,
      theme,
      injector.editor_data,
      injector.shutown,
      callback_fn,
    )
  }

  fn with(
    matcher: Nucleo<T>,
    theme: Option<Theme>,
    editor_data: Arc<T::Data>,
    shutdown: Arc<AtomicBool>,
    callback_fn: impl Fn(&mut Context, &T, Action) + 'static,
  ) -> Self {
    // let textbox = Textbox::new(
    //   "".into(),
    //   None,
    //   ui::completers::none,
    //   |_editor: &mut Context, _pattern: &str, _event: TextboxEvent| {},
    // );

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
      cursor_position: None,
      select_range: None,
    };

    Self {
      messages: Vec::new(),
      matcher,
      editor_data,
      theme,
      shutdown,
      is_focused: false,
      selected_option: 0,
      // textbox,
      state: tablestate,
      input,
      input_height,
      input_hidden: false,
      truncate_start: true,
      show_preview: true,
      callback_fn: Box::new(callback_fn),
      completion_height: 0,
      widths: Vec::new(),
      preview_cache: HashMap::new(),
      read_buffer: Vec::with_capacity(1024),
      file_fn: None,
    }
  }

  pub fn upsert_message(&mut self, message: ChatMessageItem) {
    if let Some(existing_message) =
      self.messages.iter_mut().find(|m| m.id.is_some() && m.id == message.id)
    {
      *existing_message = message;
    } else {
      self.messages.push(message);
    }
  }

  pub fn reload_messages(&mut self, messages: Vec<ChatMessageItem>) {
    self.messages = messages;
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
      if let Some((item, matcher_text)) =
        item_to_nucleo(item, &self.editor_data)
      {
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
          Some(selected) => Some(
            selected.saturating_add(amount as usize).clamp(0, len as usize - 1),
          ),
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
    self.selected_option =
      self.matcher.snapshot().matched_item_count().saturating_sub(1);
  }

  pub fn selection(&self) -> Option<&T> {
    self
      .matcher
      .snapshot()
      .get_matched_item(self.selected_option)
      .map(|item| item.data)
  }

  pub fn toggle_preview(&mut self) {
    self.show_preview = !self.show_preview;
  }

  fn prompt_handle_event(
    &mut self,
    _event: &Event,
    _cx: &mut Context,
  ) -> EventResult {
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
          .map(|(metadata, content_type)| {
            match (metadata.len(), content_type) {
              (_, content_inspector::ContentType::BINARY) => {
                CachedPreview::Binary
              },
              (size, _) if size > MAX_FILE_SIZE_FOR_PREVIEW => {
                CachedPreview::LargeFile
              },
              _ => Document::open(path, None, None, editor.config.clone())
                .map(|doc| CachedPreview::Document(Box::new(doc)))
                .unwrap_or(CachedPreview::NotFound),
            }
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
      if let Some(language_config) =
        doc.detect_language_config(&cx.editor.syn_loader.load())
      {
        doc.language = Some(language_config.clone());
        let text = doc.text().clone();
        let loader = cx.editor.syn_loader.clone();
        let job = tokio::task::spawn_blocking(move || {
          let syntax = language_config
            .highlight_config(&loader.load().scopes())
            .and_then(|highlight_config| {
              Syntax::new(text.slice(..), highlight_config, loader)
            });
          let callback =
            move |editor: &mut Editor, compositor: &mut Compositor| {
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
                log::info!(
                  "session closed before syntax highlighting finished"
                );
                return;
              };
              // Try to find a document in the cache
              let doc = match current_file {
                PathOrId::Id(doc_id) => doc_mut!(editor, &doc_id),
                PathOrId::Path(path) => {
                  match session.preview_cache.get_mut(&path) {
                    Some(CachedPreview::Document(ref mut doc)) => {
                      let diagnostics = Editor::doc_diagnostics(
                        &editor.language_servers,
                        &editor.diagnostics,
                        doc,
                      );
                      doc.replace_diagnostics(diagnostics, &[], None);
                      doc
                    },
                    _ => return,
                  }
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
  ) {
    let status = self.matcher.tick(10);
    let snapshot = self.matcher.snapshot();
    if status.changed {
      self.selected_option = self
        .selected_option
        .min(snapshot.matched_item_count().saturating_sub(1))
    }

    let text_style = cx.editor.theme.get("ui.text");
    let selected = cx.editor.theme.get("ui.selection");
    let _highlight_style =
      cx.editor.theme.get("special").add_modifier(Modifier::BOLD);

    // -- Render the frame:
    // clear area
    let background = cx.editor.theme.get("ui.background");
    surface.clear_with(area, background);

    let block = Block::default().borders(Borders::ALL);

    // calculate the inner area inside the box
    let inner = block.inner(area);

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

    // -- Render the input bar:
    let input_on_top = false;
    // define input area
    let readout_area = if self.input_hidden {
      inner.clip_top(inner.height)
    } else if input_on_top {
      inner.clip_top(self.input_height)
    } else {
      inner.clip_bottom(self.input_height)
    };

    log::info!("readout_area: {:?}", readout_area);
    // -- Separator
    if !self.input_hidden {
      // don't need the separator if the input is hidden
      let sep_height = if input_on_top {
        self.input_height
      } else {
        inner.height - self.input_height
      };

      let sep_style = cx.editor.theme.get("ui.background.separator");
      let borders = BorderType::line_symbols(BorderType::Plain);
      for x in readout_area.left()..readout_area.right() {
        if let Some(cell) = surface.get_mut(x, sep_height) {
          cell.set_symbol(borders.horizontal).set_style(sep_style);
        }
      }
    }

    // -- Render the contents:
    let mut matcher = MATCHER.lock();
    matcher.config = Config::DEFAULT;
    if self.file_fn.is_some() {
      matcher.config.set_match_paths()
    }

    let highlight_range = Some(Range {
      start: helix_lsp::Position::new(1, 0),
      end: helix_lsp::Position::new(20, 5),
    });
    let highlight_style =
      Some(Style::default().bg(Color::Yellow).fg(Color::Red));
    // let highlight_style = self.style.patch(highlight_style);

    let rows: Vec<Row> = self
      .messages
      .iter()
      .enumerate()
      .map(|(msg_idx, message_item)| {
        let text = message_item
          .format(&message_item.content().to_string(), self.theme.as_ref());
        let height = text.height() as u16;
        let message_cell = Cell::from(text).paragraph_cell(
          // Some(Block::default().borders(Borders::LEFT)),
          None,
          Some(false),
          (0, 0),
          tui::layout::Alignment::Left,
          highlight_style,
          highlight_range,
        );

        let index_cell = Cell::from(msg_idx.to_string()).paragraph_cell(
          Some(Block::default().borders(Borders::RIGHT)),
          Some(false),
          (0, 0),
          tui::layout::Alignment::Center,
          None,
          None,
        );
        Row::new(vec![index_cell, message_cell]).height(height)
      })
      .collect();

    // let options: Vec<Row> = snapshot
    //    // .matched_items(0..snapshot.matched_item_count())
    //    // .matched_items(offset..end)
    //     .matched_items(0..snapshot.item_count())
    //     // .matched_items(0..end)
    //   .map(|item| {
    //     snapshot.pattern().column_pattern(0).indices(
    //       item.matcher_columns[0].slice(..),
    //       &mut matcher,
    //       &mut indices,
    //     );
    //     indices.sort_unstable();
    //     indices.dedup();
    //     let text = item.data.format(&self.editor_data, self.theme.as_ref());
    //
    //     let height = text.height() as u16;
    //     Row::from(text).height(height)
    //   })
    //   .collect();

    self.widths = vec![Constraint::Length(5), Constraint::Percentage(25)];

    let table = Table::new(rows)
      .style(text_style)
      .highlight_style(selected)
      .highlight_symbol(" > ")
      .column_spacing(1)
      .row_spacing(1)
      .widths(&self.widths);

    table.render_table(
      readout_area,
      surface,
      &mut self.state,
      self.truncate_start,
    );
  }

  fn render_preview(
    &mut self,
    area: Rect,
    surface: &mut Surface,
    cx: &mut Context,
  ) {
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
          if range.map_or(true, |(start, end)| {
            start <= end && end <= doc.text().len_lines()
          }) =>
        {
          doc
        },
        _ => {
          let alt_text = preview.placeholder();
          let x =
            inner.x + inner.width.saturating_sub(alt_text.len() as u16) / 2;
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

      let syntax_highlights = EditorView::doc_syntax_highlights(
        doc,
        offset.anchor,
        area.height,
        &cx.editor.theme,
      );

      let mut overlay_highlights =
        EditorView::empty_highlight_iter(doc, offset.anchor, area.height);
      for spans in EditorView::doc_diagnostics_highlights(doc, &cx.editor.theme)
      {
        if spans.is_empty() {
          continue;
        }
        overlay_highlights =
          Box::new(helix_core::syntax::merge(overlay_highlights, spans));
      }
      let mut decorations: Vec<Box<dyn LineDecoration>> = Vec::new();

      if let Some((start, end)) = range {
        let style = cx
          .editor
          .theme
          .try_get("ui.highlight")
          .unwrap_or_else(|| cx.editor.theme.get("ui.selection"));
        let draw_highlight = move |renderer: &mut TextRenderer,
                                   pos: LinePos| {
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

impl<T: MarkdownItem + 'static + Send + Sync> Component for SessionView<T> {
  fn render(&mut self, area: Rect, surface: &mut Surface, cx: &mut Context) {
    // +---------+ +---------+
    // |prompt   | |preview  |
    // +---------+ |         |
    // |session   | |         |
    // |         | |         |
    // +---------+ +---------+

    let render_preview = self.show_preview
      && self.file_fn.is_some()
      && area.width > MIN_AREA_WIDTH_FOR_PREVIEW;

    let session_width =
      if render_preview { area.width / 2 } else { area.width };

    let session_area = area.with_width(session_width);
    self.render_session(session_area, surface, cx);

    if render_preview {
      let preview_area = area.clip_left(session_width);
      self.render_preview(preview_area, surface, cx);
    }
  }

  fn handle_event(&mut self, event: &Event, ctx: &mut Context) -> EventResult {
    if let Event::IdleTimeout = event {
      return self.handle_idle_timeout(ctx);
    }
    // log::info!("session events--: {:?}", event);

    let close_fn = |session: &mut Self| {
      // if the session is very large don't store it as last_session to avoid
      // excessive memory consumption
      let callback: compositor::Callback =
        if session.matcher.snapshot().item_count() > 100_000 {
          Box::new(|compositor: &mut Compositor, _ctx| {
            // remove the layer
            compositor.pop();
          })
        } else {
          // stop streaming in new items in the background, really we should
          // be restarting the stream somehow once the session gets
          // reopened instead (like for an FS crawl) that would also remove the
          // need for the special case above but that is pretty tricky
          session.shutdown.store(true, atomic::Ordering::Relaxed);
          Box::new(|compositor: &mut Compositor, _ctx| {
            // remove the layer
            compositor.last_picker = compositor.pop();
          })
        };
      EventResult::Consumed(Some(callback))
    };

    // So that idle timeout retriggers
    ctx.editor.reset_idle_timer();

    if let Event::Mouse(event) = event {
      match event.kind {
        MouseEventKind::Down(MouseButton::Left) => {
          // start select
          log::info!("mouse down event: {:?}", event);
        },
        MouseEventKind::Up(MouseButton::Left) => {
          // stop select
          log::info!("mouse up event: {:?}", event);
        },
        MouseEventKind::Drag(MouseButton::Left) => {
          // update select
          log::info!("mouse drag event: {:?}", event);
        },
        MouseEventKind::ScrollUp => {
          log::info!("scroll up");
          self.state.scroll_by(1, Direction::Backward);
          helix_event::request_redraw();
        },
        MouseEventKind::ScrollDown => {
          log::info!("scroll down");
          self.state.scroll_by(1, Direction::Forward);
          helix_event::request_redraw();
        },
        _ => {},
      }
    }

    let key_event = match event {
      Event::Key(event) => *event,
      Event::Paste(..) => return self.prompt_handle_event(event, ctx),
      Event::Resize(..) => return EventResult::Consumed(None),
      _ => {
        return EventResult::Ignored(None);
      },
    };

    log::info!("key event: {:?}", key_event);
    match key_event {
      // shift!('j') | key!(Up) => {
      //   log::info!("kb scroll up");
      //   self.state.scroll_by(1, Direction::Backward);
      //   helix_event::request_redraw();
      // },
      // shift!('k') | key!(Down) => {
      //   log::info!("kb scroll down");
      //   self.state.scroll_by(1, Direction::Forward);
      //   helix_event::request_redraw();
      // },
      // shift!(Tab) | ctrl!('p') => {
      //   self.move_by(1, Direction::Backward);
      //   log::info!("shift tab")
      // },
      // key!(Tab) | ctrl!('n') => {
      //   self.move_by(1, Direction::Forward);
      //   log::info!("tab")
      // },
      // key!(PageDown) | ctrl!('d') => {
      //   self.page_down();
      // },
      // key!(PageUp) | ctrl!('u') => {
      //   self.page_up();
      // },
      // key!(Home) => {
      //   self.to_start();
      // },
      // key!(End) => {
      //   self.to_end();
      // },
      // key!(Esc) | ctrl!('c') => return close_fn(self),
      // alt!(Enter) => {
      //   if let Some(option) = self.selection() {
      //     (self.callback_fn)(ctx, option, Action::Load);
      //   }
      // },
      // key!(Enter) => {
      //   if let Some(option) = self.selection() {
      //     (self.callback_fn)(ctx, option, Action::Replace);
      //   }
      //   return close_fn(self);
      // },
      ctrl!('s') => {
        if let Some(option) = self.selection() {
          (self.callback_fn)(ctx, option, Action::HorizontalSplit);
        }
        return close_fn(self);
      },
      ctrl!('v') => {
        if let Some(option) = self.selection() {
          (self.callback_fn)(ctx, option, Action::VerticalSplit);
        }
        return close_fn(self);
      },
      ctrl!('t') => {
        self.toggle_preview();
      },
      _ => {
        // self.editor_handle_event(event, ctx);
        // log::info!("passing event to input: {:?}", event);
        // return self.input.handle_event(event, ctx);
      },
    }

    EventResult::Consumed(None)
  }

  fn cursor(
    &self,
    area: Rect,
    editor: &Editor,
  ) -> (Option<Position>, CursorKind) {
    let block = Block::default().borders(Borders::ALL);
    // calculate the inner area inside the box
    let inner = block.inner(area);

    // prompt area
    let area = inner.clip_left(1).with_height(1);
    // self..cursor(area, editor)
    // self.textbox.cursor(area, editor)
    (Some(Position::new(area.y as usize, area.x as usize)), CursorKind::Block)
  }

  fn required_size(
    &mut self,
    (width, height): (u16, u16),
  ) -> Option<(u16, u16)> {
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
pub type DynQueryCallback<T> = Box<
  dyn Fn(String, &mut Editor) -> BoxFuture<'static, anyhow::Result<Vec<T>>>,
>;

/// A session that updates its contents via a callback whenever the
/// query string changes. Useful for live grep, workspace symbols, etc.
pub struct DynamicSession<T: MarkdownItem + Send + Sync> {
  file_session: SessionView<T>,
  query_callback: DynQueryCallback<T>,
  query: String,
}

impl<T: MarkdownItem + Send + Sync> DynamicSession<T> {
  pub fn new(
    file_session: SessionView<T>,
    query_callback: DynQueryCallback<T>,
  ) -> Self {
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

pub fn session_picker(
  root: PathBuf,
  config: &helix_view::editor::Config,
) -> Picker<PathBuf> {
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
    .filter_entry(move |entry| {
      filter_picker_entry(entry, &absolute_root, dedup_symlinks)
    });

  walk_builder
    .add_custom_ignore_filename(helix_loader::config_dir().join("ignore"));
  walk_builder.add_custom_ignore_filename(".helix/ignore");

  // We want to exclude files that the editor can't handle yet
  let mut type_builder = TypesBuilder::new();
  type_builder
    .add(
      "compressed",
      "*.{zip,gz,bz2,zst,lzo,sz,tgz,tbz2,lz,lz4,lzma,lzo,z,Z,xz,7z,rar,cab}",
    )
    .expect("Invalid type definition");
  type_builder.negate("all");
  let excluded_types =
    type_builder.build().expect("failed to build excluded_types");
  walk_builder.types(excluded_types);
  let mut files = walk_builder.build().filter_map(|entry| {
    let entry = entry.ok()?;
    if !entry.file_type()?.is_file() {
      return None;
    }
    Some(entry.into_path())
  });
  log::debug!("file_picker init {:?}", Instant::now().duration_since(now));

  let picker =
    Picker::new(Vec::new(), root, move |cx, path: &PathBuf, _action| {
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
  let timeout =
    std::time::Instant::now() + std::time::Duration::from_millis(30);

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
