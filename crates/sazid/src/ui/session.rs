use std::{mem::take, num::NonZeroUsize, path::PathBuf, rc::Rc, sync::Arc};

use crate::commands::{self, OnKeyCallback};
use crate::components::session::Session;
use crate::compositor::{Component, Context, Event, EventResult};
use crate::keymap::{macros::*, KeymapResult, Keymaps};
use helix_core::{
  diagnostic::NumberOrString,
  graphemes::{
    ensure_grapheme_boundary_next_byte, next_grapheme_boundary,
    prev_grapheme_boundary,
  },
  movement::Direction,
  syntax::{self, HighlightEvent},
  text_annotations::TextAnnotations,
  unicode::width::UnicodeWidthStr,
  visual_offset_from_block, Change, Position, Range, Selection, Transaction,
};
use helix_view::{
  document::{Mode, SavePoint, SCRATCH_BUFFER_NAME},
  editor::{CompleteAction, CursorShapeConfig},
  graphics::{Color, CursorKind, Margin, Modifier, Rect, Style},
  input::{KeyEvent, MouseButton, MouseEvent, MouseEventKind},
  keyboard::{KeyCode, KeyModifiers},
  Document, Theme, View,
};
use tui::buffer::Buffer as Surface;
use tui::widgets::{Block, Borders, Paragraph, Widget};

use super::{Completion, CompletionItem, ProgressSpinners};

pub struct SessionView {
  pub keymaps: Keymaps,
  on_next_key: Option<OnKeyCallback>,
  pseudo_pending: Vec<KeyEvent>,
  pub(crate) last_insert: (commands::MappableCommand, Vec<InsertEvent>),
  pub(crate) completion: Option<Completion>,
  spinners: ProgressSpinners,
  /// Tracks if the terminal window is focused by reaction to terminal focus events
  terminal_focused: bool,
}

#[derive(Debug, Clone)]
pub enum InsertEvent {
  Key(KeyEvent),
  CompletionApply { trigger_offset: usize, changes: Vec<Change> },
  TriggerCompletion,
  RequestCompletion,
}

impl Default for SessionView {
  fn default() -> Self {
    Self::new(Keymaps::default())
  }
}

impl SessionView {
  pub fn new(keymaps: Keymaps) -> Self {
    Self {
      keymaps,
      on_next_key: None,
      pseudo_pending: Vec::new(),
      last_insert: (commands::MappableCommand::normal_mode, Vec::new()),
      completion: None,
      spinners: ProgressSpinners::default(),
      terminal_focused: true,
    }
  }

  pub fn spinners_mut(&mut self) -> &mut ProgressSpinners {
    &mut self.spinners
  }

  pub fn render_view(
    &self,
    session: &Session,
    doc: &Document,
    view: &View,
    viewport: Rect,
    surface: &mut Surface,
    is_focused: bool,
  ) {
  }

  fn insert_mode(&mut self, cx: &mut commands::Context, event: KeyEvent) {
    if let Some(keyresult) = self.handle_keymap_event(Mode::Insert, cx, event) {
      match keyresult {
        KeymapResult::NotFound => {
          if let Some(ch) = event.char() {
            commands::insert::insert_char(cx, ch)
          }
        },
        KeymapResult::Cancelled(pending) => {
          for ev in pending {
            match ev.char() {
              Some(ch) => commands::insert::insert_char(cx, ch),
              None => {
                if let KeymapResult::Matched(command) =
                  self.keymaps.get(Mode::Insert, ev)
                {
                  command.execute(cx);
                }
              },
            }
          }
        },
        _ => unreachable!(),
      }
    }
  }

  fn command_mode(
    &mut self,
    mode: Mode,
    cxt: &mut commands::Context,
    event: KeyEvent,
  ) {
    match (event, cxt.session.count) {
      // count handling
      (key!(i @ '0'), Some(_)) | (key!(i @ '1'..='9'), _)
        if !self.keymaps.contains_key(mode, event) =>
      {
        let i = i.to_digit(10).unwrap() as usize;
        cxt.session.count = std::num::NonZeroUsize::new(
          cxt.session.count.map_or(i, |c| c.get() * 10 + i),
        );
      },
      // special handling for repeat operator
      (key!('.'), _) if self.keymaps.pending().is_empty() => {
        for _ in 0..cxt.session.count.map_or(1, NonZeroUsize::into) {
          // first execute whatever put us into insert mode
          self.last_insert.0.execute(cxt);
          let mut last_savepoint = None;
          let mut last_request_savepoint = None;
          // then replay the inputs
          for key in self.last_insert.1.clone() {
            match key {
              InsertEvent::Key(key) => self.insert_mode(cxt, key),
              InsertEvent::CompletionApply { trigger_offset, changes } => {
                let (view, doc) = current!(cxt.session);

                if let Some(last_savepoint) = last_savepoint.as_deref() {
                  doc.restore(view, last_savepoint, true);
                }

                let text = doc.text().slice(..);
                let cursor = doc.selection(view.id).primary().cursor(text);

                let shift_position = |pos: usize| -> usize {
                  (pos + cursor).saturating_sub(trigger_offset)
                };

                let tx = Transaction::change(
                  doc.text(),
                  changes.iter().cloned().map(|(start, end, t)| {
                    (shift_position(start), shift_position(end), t)
                  }),
                );
                doc.apply(&tx, view.id);
              },
              InsertEvent::TriggerCompletion => {
                last_savepoint = take(&mut last_request_savepoint);
              },
              InsertEvent::RequestCompletion => {
                let (view, doc) = current!(cxt.session);
                last_request_savepoint = Some(doc.savepoint(view));
              },
            }
          }
        }
        cxt.session.count = None;
      },
      _ => {
        // set the count
        cxt.count = cxt.session.count;
        // TODO: edge case: 0j -> reset to 1
        // if this fails, count was Some(0)
        // debug_assert!(cxt.count != 0);

        // set the register
        cxt.register = cxt.session.selected_register.take();

        self.handle_keymap_event(mode, cxt, event);
        if self.keymaps.pending().is_empty() {
          cxt.session.count = None
        } else {
          cxt.session.selected_register = cxt.register.take();
        }
      },
    }
  }

  #[allow(clippy::too_many_arguments)]
  pub fn set_completion(
    &mut self,
    session: &mut Session,
    savepoint: Arc<SavePoint>,
    items: Vec<CompletionItem>,
    trigger_offset: usize,
    size: Rect,
  ) -> Option<Rect> {
    let mut completion =
      Completion::new(session, savepoint, items, trigger_offset);

    if completion.is_empty() {
      // skip if we got no completion results
      return None;
    }

    let area = completion.area(size, session);
    session.last_completion = Some(CompleteAction::Triggered);
    self.last_insert.1.push(InsertEvent::TriggerCompletion);

    // TODO : propagate required size on resize to completion too
    completion.required_size((size.width, size.height));
    self.completion = Some(completion);
    Some(area)
  }

  pub fn clear_completion(&mut self, session: &mut Session) {
    self.completion = None;
    if let Some(last_completion) = session.last_completion.take() {
      match last_completion {
        CompleteAction::Triggered => (),
        CompleteAction::Applied { trigger_offset, changes } => self
          .last_insert
          .1
          .push(InsertEvent::CompletionApply { trigger_offset, changes }),
        CompleteAction::Selected { savepoint } => {
          let (view, doc) = current!(session);
          doc.restore(view, &savepoint, false);
        },
      }
    }
  }

  pub fn handle_idle_timeout(
    &mut self,
    cx: &mut commands::Context,
  ) -> EventResult {
    commands::compute_inlay_hints_for_all_views(cx.session, cx.jobs);

    EventResult::Ignored(None)
  }

  fn handle_mouse_event(
    &mut self,
    event: &MouseEvent,
    cxt: &mut commands::Context,
  ) -> EventResult {
    if event.kind != MouseEventKind::Moved {
      cxt.session.reset_idle_timer();
    }
    todo!();
    EventResult::Ignored(None)
  }
}

fn viewport_byte_range(
  text: helix_core::RopeSlice,
  row: usize,
  height: u16,
) -> std::ops::Range<usize> {
  // Calculate viewport byte ranges:
  // Saturating subs to make it inclusive zero indexing.
  let last_line = text.len_lines().saturating_sub(1);
  let last_visible_line =
    (row + height as usize).saturating_sub(1).min(last_line);
  let start = text.line_to_byte(row.min(last_line));
  let end = text.line_to_byte(last_visible_line + 1);

  start..end
}

impl Component for SessionView {
  fn handle_event(
    &mut self,
    event: &Event,
    context: &mut Context,
  ) -> EventResult {
    let mut cx = commands::Context {
      session: context.session,
      count: None,
      register: None,
      callback: Vec::new(),
      on_next_key_callback: None,
      jobs: context.jobs,
    };

    match event {
      Event::Paste(contents) => {},
      _ => {},
    }
  }

  fn render(
    &mut self,
    viewport: Rect,
    surface: &mut Surface,
    cx: &mut Context,
  ) {
    let text_style = cx.session.theme.get("ui.text.info");
    let popup_style = cx.session.theme.get("ui.popup.info");

    // Calculate the area of the terminal to modify. Because we want to
    // render at the bottom right, we use the viewport's width and height
    // which evaluate to the most bottom right coordinate.
    let width = self.width + 2 + 2; // +2 for border, +2 for margin
    let height = self.height + 2; // +2 for border
    let area = viewport.intersection(Rect::new(
      viewport.width.saturating_sub(width),
      viewport.height.saturating_sub(height + 2), // +2 for statusline
      width,
      height,
    ));
    surface.clear_with(area, popup_style);

    let block = Block::default()
      .title(self.title.as_str())
      .borders(Borders::ALL)
      .border_style(popup_style);

    let margin = Margin::horizontal(1);
    let inner = block.inner(area).inner(&margin);
    block.render(area, surface);

    Paragraph::new(self.text.as_str()).style(text_style).render(inner, surface);
  }
}
