use std::{borrow::Cow, cmp::Reverse, path::PathBuf};

use crate::{
  components::session::Session,
  compositor::{Callback, Component, Compositor, Context, Event, EventResult},
  ctrl, key, shift,
};
use helix_core::fuzzy::MATCHER;
use nucleo::pattern::{Atom, AtomKind, CaseMatching};
use nucleo::{Config, Utf32Str};
use tui::{
  buffer::Buffer as Surface,
  widgets::{Block, Borders, Table, Widget},
};

pub use tui::widgets::{Cell, Row};

use helix_view::{
  editor::SmartTabConfig,
  graphics::{Margin, Rect},
};

use tui::layout::Constraint;

pub trait Item: Sync + Send + 'static {
  /// Additional session state that is used for label calculation.
  type Data: Sync + Send + 'static;

  fn format(&self, data: &Self::Data) -> Row;

  fn sort_text(&self, data: &Self::Data) -> Cow<str> {
    let label: String = self.format(data).cell_text().collect();
    label.into()
  }

  fn filter_text(&self, data: &Self::Data) -> Cow<str> {
    let label: String = self.format(data).cell_text().collect();
    label.into()
  }
}

impl Item for PathBuf {
  /// Root prefix to strip.
  type Data = PathBuf;

  fn format(&self, root_path: &Self::Data) -> Row {
    self.strip_prefix(root_path).unwrap_or(self).to_string_lossy().into()
  }
}

pub type MenuCallback<T> = Box<dyn Fn(&mut Session, Option<&T>, MenuEvent)>;

pub struct Menu<T: Item> {
  options: Vec<T>,
  session_data: T::Data,

  cursor: Option<usize>,

  /// (index, score)
  matches: Vec<(u32, u32)>,

  widths: Vec<Constraint>,

  callback_fn: MenuCallback<T>,

  scroll: usize,
  size: (u16, u16),
  viewport: (u16, u16),
  recalculate: bool,
}

impl<T: Item> Menu<T> {
  const LEFT_PADDING: usize = 1;

  // TODO: it's like a slimmed down picker, share code? (picker = menu + prompt with different
  // rendering)
  pub fn new(
    options: Vec<T>,
    session_data: <T as Item>::Data,
    callback_fn: impl Fn(&mut Session, Option<&T>, MenuEvent) + 'static,
  ) -> Self {
    let matches = (0..options.len() as u32).map(|i| (i, 0)).collect();
    Self {
      options,
      session_data,
      matches,
      cursor: None,
      widths: Vec::new(),
      callback_fn: Box::new(callback_fn),
      scroll: 0,
      size: (0, 0),
      viewport: (0, 0),
      recalculate: true,
    }
  }

  pub fn score(&mut self, pattern: &str, incremental: bool) {
    let mut matcher = MATCHER.lock();
    matcher.config = Config::DEFAULT;
    let pattern =
      Atom::new(pattern, CaseMatching::Ignore, AtomKind::Fuzzy, false);
    let mut buf = Vec::new();
    if incremental {
      self.matches.retain_mut(|(index, score)| {
        let option = &self.options[*index as usize];
        let text = option.filter_text(&self.session_data);
        let new_score =
          pattern.score(Utf32Str::new(&text, &mut buf), &mut matcher);
        match new_score {
          Some(new_score) => {
            *score = new_score as u32;
            true
          },
          None => false,
        }
      })
    } else {
      self.matches.clear();
      let matches =
        self.options.iter().enumerate().filter_map(|(i, option)| {
          let text = option.filter_text(&self.session_data);
          pattern
            .score(Utf32Str::new(&text, &mut buf), &mut matcher)
            .map(|score| (i as u32, score as u32))
        });
      self.matches.extend(matches);
    }
    self.matches.sort_unstable_by_key(|&(i, score)| (Reverse(score), i));

    // reset cursor position
    self.cursor = None;
    self.scroll = 0;
    self.recalculate = true;
  }

  pub fn clear(&mut self) {
    self.matches.clear();

    // reset cursor position
    self.cursor = None;
    self.scroll = 0;
  }

  pub fn move_up(&mut self) {
    let len = self.matches.len();
    let max_index = len.saturating_sub(1);
    let pos = self.cursor.map_or(max_index, |i| (i + max_index) % len) % len;
    self.cursor = Some(pos);
    self.adjust_scroll();
  }

  pub fn move_down(&mut self) {
    let len = self.matches.len();
    let pos = self.cursor.map_or(0, |i| i + 1) % len;
    self.cursor = Some(pos);
    self.adjust_scroll();
  }

  fn recalculate_size(&mut self, viewport: (u16, u16)) {
    let n = self
      .options
      .first()
      .map(|option| option.format(&self.session_data).cells.len())
      .unwrap_or_default();
    let max_lens = self.options.iter().fold(vec![0; n], |mut acc, option| {
      let row = option.format(&self.session_data);
      // maintain max for each column
      for (acc, cell) in acc.iter_mut().zip(row.cells.iter()) {
        let width = cell.content.width();
        if width > *acc {
          *acc = width;
        }
      }

      acc
    });

    let height = self.matches.len().min(10).min(viewport.1 as usize);
    // do all the matches fit on a single screen?
    let fits = self.matches.len() <= height;

    let mut len = max_lens.iter().sum::<usize>() + n;

    if !fits {
      len += 1; // +1: reserve some space for scrollbar
    }

    len += Self::LEFT_PADDING;
    let width = len.min(viewport.0 as usize);

    self.widths =
      max_lens.into_iter().map(|len| Constraint::Length(len as u16)).collect();

    self.size = (width as u16, height as u16);

    // adjust scroll offsets if size changed
    self.adjust_scroll();
    self.recalculate = false;
  }

  fn adjust_scroll(&mut self) {
    let win_height = self.size.1 as usize;
    if let Some(cursor) = self.cursor {
      let mut scroll = self.scroll;
      if cursor > (win_height + scroll).saturating_sub(1) {
        // scroll down
        scroll += cursor - (win_height + scroll).saturating_sub(1)
      } else if cursor < scroll {
        // scroll up
        scroll = cursor
      }
      self.scroll = scroll;
    }
  }

  pub fn selection(&self) -> Option<&T> {
    self.cursor.and_then(|cursor| {
      self
        .matches
        .get(cursor)
        .map(|(index, _score)| &self.options[*index as usize])
    })
  }

  pub fn selection_mut(&mut self) -> Option<&mut T> {
    self.cursor.and_then(|cursor| {
      self
        .matches
        .get(cursor)
        .map(|(index, _score)| &mut self.options[*index as usize])
    })
  }

  pub fn is_empty(&self) -> bool {
    self.matches.is_empty()
  }

  pub fn len(&self) -> usize {
    self.matches.len()
  }
}

impl<T: Item + PartialEq> Menu<T> {
  pub fn replace_option(&mut self, old_option: T, new_option: T) {
    for option in &mut self.options {
      if old_option == *option {
        *option = new_option;
        break;
      }
    }
  }
}

use super::PromptEvent as MenuEvent;

impl<T: Item + 'static> Component for Menu<T> {
  fn handle_event(&mut self, event: &Event, cx: &mut Context) -> EventResult {
    let event = match event {
      Event::Key(event) => *event,
      _ => return EventResult::Ignored(None),
    };

    let close_fn: Option<Callback> =
      Some(Box::new(|compositor: &mut Compositor, _| {
        // remove the layer
        compositor.pop();
      }));

    // Ignore tab key when supertab is turned on in order not to interfere
    // with it. (Is there a better way to do this?)
    if (event == key!(Tab) || event == shift!(Tab))
      && cx.session.config().auto_completion
      && matches!(
        cx.session.config().smart_tab,
        Some(SmartTabConfig { enable: true, supersede_menu: true })
      )
    {
      return EventResult::Ignored(None);
    }

    match event {
      // esc or ctrl-c aborts the completion and closes the menu
      key!(Esc) | ctrl!('c') => {
        (self.callback_fn)(cx.session, self.selection(), MenuEvent::Abort);
        return EventResult::Consumed(close_fn);
      },
      // arrow up/ctrl-p/shift-tab prev completion choice (including updating the doc)
      shift!(Tab) | key!(Up) | ctrl!('p') => {
        self.move_up();
        (self.callback_fn)(cx.session, self.selection(), MenuEvent::Update);
        return EventResult::Consumed(None);
      },
      key!(Tab) | key!(Down) | ctrl!('n') => {
        // arrow down/ctrl-n/tab advances completion choice (including updating the doc)
        self.move_down();
        (self.callback_fn)(cx.session, self.selection(), MenuEvent::Update);
        return EventResult::Consumed(None);
      },
      key!(Enter) => {
        if let Some(selection) = self.selection() {
          (self.callback_fn)(cx.session, Some(selection), MenuEvent::Validate);
          return EventResult::Consumed(close_fn);
        } else {
          return EventResult::Ignored(close_fn);
        }
      },
      // KeyEvent {
      //     code: KeyCode::Char(c),
      //     modifiers: KeyModifiers::NONE,
      // } => {
      //     self.insert_char(c);
      //     (self.callback_fn)(cx.session, &self.line, MenuEvent::Update);
      // }

      // / -> edit_filter?
      //
      // enter confirms the match and closes the menu
      // typing filters the menu
      // if we run out of options the menu closes itself
      _ => (),
    }
    // for some events, we want to process them but send ignore, specifically all input except
    // tab/enter/ctrl-k or whatever will confirm the selection/ ctrl-n/ctrl-p for scroll.
    // EventResult::Consumed(None)
    EventResult::Ignored(None)
  }

  fn required_size(&mut self, viewport: (u16, u16)) -> Option<(u16, u16)> {
    if viewport != self.viewport || self.recalculate {
      self.recalculate_size(viewport);
    }

    Some(self.size)
  }

  fn render(&mut self, area: Rect, surface: &mut Surface, cx: &mut Context) {
    let theme = &cx.session.theme;
    let style =
      theme.try_get("ui.menu").unwrap_or_else(|| theme.get("ui.text"));
    let selected = theme.get("ui.menu.selected");
    surface.clear_with(area, style);

    let render_borders = cx.session.menu_border();

    let area = if render_borders {
      Widget::render(Block::default().borders(Borders::ALL), area, surface);
      area.inner(&Margin::vertical(1))
    } else {
      area
    };

    let scroll = self.scroll;

    let options: Vec<_> = self
      .matches
      .iter()
      .map(|(index, _score)| {
        // (index, self.options.get(*index).unwrap()) // get_unchecked
        &self.options[*index as usize] // get_unchecked
      })
      .collect();

    let len = options.len();

    let win_height = area.height as usize;

    const fn div_ceil(a: usize, b: usize) -> usize {
      (a + b - 1) / b
    }

    let rows = options.iter().map(|option| option.format(&self.session_data));
    let table = Table::new(rows)
      .style(style)
      .highlight_style(selected)
      .column_spacing(1)
      .widths(&self.widths);

    use tui::widgets::TableState;

    table.render_table(
      area.clip_left(Self::LEFT_PADDING as u16).clip_right(1),
      surface,
      &mut TableState { offset: scroll, selected: self.cursor },
      false,
    );

    let render_borders = cx.session.menu_border();

    if !render_borders {
      if let Some(cursor) = self.cursor {
        let offset_from_top = cursor - scroll;
        let left = &mut surface[(area.left(), area.y + offset_from_top as u16)];
        left.set_style(selected);
        let right = &mut surface
          [(area.right().saturating_sub(1), area.y + offset_from_top as u16)];
        right.set_style(selected);
      }
    }

    let fits = len <= win_height;

    let scroll_style = theme.get("ui.menu.scroll");
    if !fits {
      let scroll_height = div_ceil(win_height.pow(2), len).min(win_height);
      let scroll_line = (win_height - scroll_height) * scroll
        / std::cmp::max(1, len.saturating_sub(win_height));

      let mut cell;
      for i in 0..win_height {
        cell = &mut surface[(area.right() - 1, area.top() + i as u16)];

        let half_block = if render_borders { "▌" } else { "▐" };

        if scroll_line <= i && i < scroll_line + scroll_height {
          // Draw scroll thumb
          cell.set_symbol(half_block);
          cell
            .set_fg(scroll_style.fg.unwrap_or(helix_view::theme::Color::Reset));
        } else if !render_borders {
          // Draw scroll track
          cell.set_symbol(half_block);
          cell
            .set_fg(scroll_style.bg.unwrap_or(helix_view::theme::Color::Reset));
        }
      }
    }
  }
}