use std::{iter, sync::Arc};

use arc_swap::ArcSwap;
use helix_core::{
  movement::Direction,
  syntax::{self},
  unicode::width::UnicodeWidthStr,
  Position, Rope,
};
use helix_lsp::lsp::Range;
use helix_view::{
  graphics::{Rect, Style},
  Theme,
};
use tui::{
  buffer::Buffer,
  layout::{Alignment, Constraint},
  text::{StyledGrapheme, Text},
  widgets::{Block, Widget},
};

use crate::{
  commands::ChatMessageItem,
  widgets::reflow::{LineComposer, LineTruncator, WordWrapper},
};

use super::paragraph::Wrap;

/// A [`Cell`] contains the [`Text`] to be displayed in a [`Row`] of a [`Table`].
///
/// It can be created from anything that can be converted to a [`Text`].
/// ```rust
/// # use helix_tui::widgets::Cell;
/// # use helix_tui::text::{Span, Spans, Text};
/// # use helix_view::graphics::{Style, Modifier};
/// Cell::from("simple string");
///
/// Cell::from(Span::from("span"));
///
/// Cell::from(Spans::from(vec![
///     Span::raw("a vec of "),
///     Span::styled("spans", Style::default().add_modifier(Modifier::BOLD))
/// ]));
///
/// Cell::from(Text::from("a text"));
/// ```
///
/// You can apply a [`Style`] on the entire [`Cell`] using [`Cell::style`] or rely on the styling
/// capabilities of [`Text`].

#[derive(Debug, Clone, PartialEq)]
pub enum MessageType<'a> {
  Chat(&'a ChatMessageItem),
  Text(String),
}
#[derive(Debug, Clone, PartialEq)]
pub struct MessageCell<'a> {
  /// The text to display
  pub message: MessageType<'a>,
  /// Widget style
  style: Style,
  /// A block to wrap the widget in
  block: Option<Block<'a>>,
  /// Highlight style
  highlight_style: Option<Style>,
  /// Highlight Range
  highlight_range: Option<std::ops::Range<usize>>,
  /// char index range
  char_idx: Option<usize>,
  /// How to wrap the text
  wrap_trim: Option<bool>,
  /// Alignment of the text
  alignment: Alignment,
}

impl<'a> MessageCell<'a> {
  pub fn get_height(&self, width: u16) -> u16 {
    match &self.message {
      MessageType::Chat(message) => message.get_wrapped_height(width) as u16,
      MessageType::Text(s) => s.lines().count() as u16,
    }
  }
  pub fn new(message: MessageType<'a>) -> Self {
    MessageCell {
      message,
      style: Style::default(),
      block: None,
      highlight_style: None,
      highlight_range: None,
      char_idx: None,
      wrap_trim: None,
      alignment: Alignment::Left,
    }
  }

  /// Set the `Style` of this cell.
  pub fn with_style(mut self, style: Style) -> Self {
    self.style = style;
    self
  }

  pub fn with_block(mut self, block: Block<'a>) -> Self {
    self.block = Some(block);
    self
  }

  pub fn with_highlight(mut self, style: Style, range: std::ops::Range<usize>) -> Self {
    self.highlight_style = Some(style);
    self.highlight_range = Some(range);
    self
  }

  pub fn with_char_index(mut self, char_idx: usize) -> Self {
    self.char_idx = Some(char_idx);
    self
  }

  pub fn with_wrap_trim(mut self, wrap_trim: bool) -> Self {
    self.wrap_trim = Some(wrap_trim);
    self
  }

  pub fn centered(mut self) -> Self {
    self.alignment = Alignment::Center;
    self
  }

  pub fn render_cell(
    &self,
    buf: &mut Buffer,
    area: Rect,
    theme: &Theme,
    skip_lines: u16,
    config_loader: &Arc<ArcSwap<syntax::Loader>>,
  ) {
    let text = match &self.message {
      MessageType::Chat(message) => message.format_to_text(Some(theme), config_loader.clone()),
      MessageType::Text(text) => Text::from(text.clone()),
    };
    let style = Style::default();
    let _scroll = (0, 0);
    Self::format_text(
      buf,
      false,
      true,
      &text,
      style,
      self.wrap_trim.map(|trim| Wrap { trim }),
      area,
      self.alignment,
      self.char_idx,
      skip_lines,
      self.highlight_range.clone(),
      self.highlight_style,
    );
  }

  #[allow(clippy::too_many_arguments)]
  pub fn format_text(
    buf: &mut Buffer,
    output_plain_text: bool,
    output_buffer: bool,
    text: &Text<'_>,
    style: Style,
    wrap: Option<Wrap>,
    area: Rect,
    _alignment: Alignment,
    char_idx: Option<usize>,
    skip_lines: u16,
    highlight_range: Option<std::ops::Range<usize>>,
    highlight_style: Option<Style>,
  ) -> Option<Rope> {
    let mut styled = text.lines.iter().flat_map(|spans| {
      spans
            .0
            .iter()
            .flat_map(|span| span.styled_graphemes(style))
            // Required given the way composers work but might be refactored out if we change
            // composers to operate on lines instead of a stream of graphemes.
            .chain(iter::once(StyledGrapheme {
                symbol: "\n",
                style
            }))
    });
    let mut line_composer: Box<dyn LineComposer> = if let Some(Wrap { trim }) = wrap {
      Box::new(WordWrapper::new(&mut styled, area.width, trim))
    } else {
      let line_composer = Box::new(LineTruncator::new(&mut styled, area.width));
      // if alignment == Alignment::Left {
      //   line_composer.set_horizontal_offset(scroll.1);
      // }
      line_composer
    };

    let mut plain_text = String::new();
    let mut line_buffer = String::new();

    let mut y = -(skip_lines as i16);
    let mut char_counter = char_idx.unwrap_or(0);
    while let Some((current_line, current_line_width)) = line_composer.next_line() {
      let mut x = 0;
      let mut linelens = vec![];
      let idx_start = char_counter;
      for (StyledGrapheme { symbol, style }, grapheme_index) in current_line.iter().zip(idx_start..)
      {
        line_buffer.push_str(symbol);
        linelens.push(symbol.width());
        let style = if let (Some(highlight_range), Some(highlight_style)) =
          (highlight_range.as_ref(), highlight_style)
        {
          if highlight_range.contains(&grapheme_index) {
            highlight_style
          } else {
            *style
          }
        } else {
          *style
        };
        if output_plain_text {
          plain_text.push_str(symbol);
        }
        if output_buffer && y >= 0 {
          let cell = &mut buf[(area.left() + x, area.top() + y as u16)];
          cell.set_symbol(symbol).set_style(style);
        }
        x += symbol.width() as u16;
      }
      char_counter += current_line_width as usize + 1;

      if output_plain_text {
        plain_text.push('\n');
        line_buffer.clear();
      }
      y += 1;
      if output_buffer && y >= area.height as i16 {
        break;
      }
    }
    if output_plain_text {
      plain_text.pop();
    }
    match output_plain_text {
      true => Some(Rope::from(plain_text)),
      false => None,
    }
  }
}
// where
//   T: Into<Text<'a>>,
// {
//   fn from(content: T) -> MessageCell<'a> {
//     MessageCell {
//       char_idx: None,
//       content: content.into(),
//       style: Style::default(),
//       paragraph_options: None,
//     }
//   }
// }

/// Holds data to be displayed in a [`Table`] widget.
///
/// A [`Row`] is a collection of cells. It can be created from simple strings:
/// ```rust
/// # use helix_tui::widgets::Row;
/// Row::new(vec!["Cell1", "Cell2", "Cell3"]);
/// ```
///
/// But if you need a bit more control over individual cells, you can explicitly create [`Cell`]s:
/// ```rust
/// # use helix_tui::widgets::{Row, Cell};
/// # use helix_view::graphics::{Style, Color};
/// Row::new(vec![
///     Cell::from("Cell1"),
///     Cell::from("Cell2").style(Style::default().fg(Color::Yellow)),
/// ]);
/// ```
///
/// By default, a row has a height of 1 but you can change this using [`Row::height`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Row<'a> {
  pub cells: Vec<MessageCell<'a>>,
  height: u16,
  style: Style,
  bottom_margin: u16,
}

impl<'a> Row<'a> {
  /// Creates a new [`Row`] from an iterator where items can be converted to a [`Cell`].
  pub fn new<T>(cells: T) -> Self
  where
    T: IntoIterator,
    T::Item: Into<MessageCell<'a>>,
  {
    Self {
      height: 1,
      cells: cells.into_iter().map(|c| c.into()).collect(),
      style: Style::default(),
      bottom_margin: 0,
    }
  }

  /// Set the fixed height of the [`Row`]. Any [`Cell`] whose content has more lines than this
  /// height will see its content truncated.
  pub fn height(mut self, height: u16) -> Self {
    self.height = height;
    self
  }

  /// Set the [`Style`] of the entire row. This [`Style`] can be overridden by the [`Style`] of a
  /// any individual [`Cell`] or event by their [`Text`] content.
  pub fn style(mut self, style: Style) -> Self {
    self.style = style;
    self
  }

  /// Set the bottom margin. By default, the bottom margin is `0`.
  pub fn bottom_margin(mut self, margin: u16) -> Self {
    self.bottom_margin = margin;
    self
  }

  /// Returns the total height of the row.
  fn total_height(&self) -> u16 {
    self.height.saturating_add(self.bottom_margin)
  }

  /// Returns the contents of cells as plain text, without styles and colors.
  pub fn cell_text(&self) -> impl Iterator<Item = String> + '_ {
    self.cells.iter().map(|cell| match &cell.message {
      MessageType::Chat(message) => message.plain_text.to_string(),
      MessageType::Text(s) => s.to_string(),
    })
  }

  /// Update height to cell max height, for a specific cell width
  pub fn update_wrapped_heights(&mut self, column_widths: Vec<u16>) {
    self.cells.iter_mut().zip(column_widths.iter()).for_each(|(cell, width)| {
      let cell_height = cell.get_height(*width);
      if cell_height > self.height {
        self.height = cell_height;
      }
    });
  }
}

impl<'a, T: Into<MessageCell<'a>>> From<T> for Row<'a> {
  fn from(cell: T) -> Self {
    Row::new(vec![cell.into()])
  }
}

/// A widget to display data in formatted columns.
///
/// It is a collection of [`Row`]s, themselves composed of [`Cell`]s:
/// ```rust
/// # use helix_tui::widgets::{Block, Borders, Table, Row, Cell};
/// # use helix_tui::layout::Constraint;
/// # use helix_view::graphics::{Style, Color, Modifier};
/// # use helix_tui::text::{Text, Spans, Span};
/// Table::new(vec![
///     // Row can be created from simple strings.
///     Row::new(vec!["Row11", "Row12", "Row13"]),
///     // You can style the entire row.
///     Row::new(vec!["Row21", "Row22", "Row23"]).style(Style::default().fg(Color::Blue)),
///     // If you need more control over the styling you may need to create Cells directly
///     Row::new(vec![
///         Cell::from("Row31"),
///         Cell::from("Row32").style(Style::default().fg(Color::Yellow)),
///         Cell::from(Spans::from(vec![
///             Span::raw("Row"),
///             Span::styled("33", Style::default().fg(Color::Green))
///         ])),
///     ]),
///     // If a Row need to display some content over multiple lines, you just have to change
///     // its height.
///     Row::new(vec![
///         Cell::from("Row\n41"),
///         Cell::from("Row\n42"),
///         Cell::from("Row\n43"),
///     ]).height(2),
/// ])
/// // You can set the style of the entire Table.
/// .style(Style::default().fg(Color::White))
/// // It has an optional header, which is simply a Row always visible at the top.
/// .header(
///     Row::new(vec!["Col1", "Col2", "Col3"])
///         .style(Style::default().fg(Color::Yellow))
///         // If you want some space between the header and the rest of the rows, you can always
///         // specify some margin at the bottom.
///         .bottom_margin(1)
/// )
/// // As any other widget, a Table can be wrapped in a Block.
/// .block(Block::default().title("Table"))
/// // Columns widths are constrained in the same way as Layout...
/// .widths(&[Constraint::Length(5), Constraint::Length(5), Constraint::Length(10)])
/// // ...and they can be separated by a fixed spacing.
/// .column_spacing(1)
/// // If you wish to highlight a row in any specific way when it is selected...
/// .highlight_style(Style::default().add_modifier(Modifier::BOLD))
/// // ...and potentially show a symbol in front of the selection.
/// .highlight_symbol(">>");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Table<'a> {
  /// A block to wrap the widget in
  block: Option<Block<'a>>,
  /// Base style for the widget
  style: Style,
  /// Width constraints for each column
  widths: &'a [Constraint],
  /// Space between each row
  row_spacing: u16,
  /// Space between each column
  column_spacing: u16,
  /// Style used to render the selected row
  highlight_style: Style,
  /// Symbol in front of the selected rom
  highlight_symbol: Option<&'a str>,
  /// Optional header
  header: Option<Row<'a>>,
  /// Data to display in each row
  rows: Vec<Row<'a>>,
  cursor_position: Option<Position>,
  cursor_style: Option<Style>,
}

impl<'a> Table<'a> {
  pub fn new<T>(rows: T) -> Self
  where
    T: IntoIterator<Item = Row<'a>>,
  {
    Self {
      block: None,
      style: Style::default(),
      widths: &[],
      column_spacing: 1,
      cursor_position: None,
      cursor_style: None,
      row_spacing: 0,
      highlight_style: Style::default(),
      highlight_symbol: None,
      header: None,
      rows: rows.into_iter().collect(),
    }
  }

  pub fn cursor_position(mut self, position: Position) -> Self {
    self.cursor_position = Some(position);
    self
  }
  pub fn cursor_style(mut self, style: Style) -> Self {
    self.cursor_style = Some(style);
    self
  }
  pub fn block(mut self, block: Block<'a>) -> Self {
    self.block = Some(block);
    self
  }

  pub fn header(mut self, header: Row<'a>) -> Self {
    self.header = Some(header);
    self
  }

  pub fn widths(mut self, widths: &'a [Constraint]) -> Self {
    let between_0_and_100 = |&w| match w {
      Constraint::Percentage(p) => p <= 100,
      _ => true,
    };
    assert!(
      widths.iter().all(between_0_and_100),
      "Percentages should be between 0 and 100 inclusively."
    );
    self.widths = widths;
    self
  }

  pub fn style(mut self, style: Style) -> Self {
    self.style = style;
    self
  }

  pub fn highlight_symbol(mut self, highlight_symbol: &'a str) -> Self {
    self.highlight_symbol = Some(highlight_symbol);
    self
  }

  pub fn highlight_style(mut self, highlight_style: Style) -> Self {
    self.highlight_style = highlight_style;
    self
  }

  pub fn column_spacing(mut self, spacing: u16) -> Self {
    self.column_spacing = spacing;
    self
  }

  pub fn row_spacing(mut self, spacing: u16) -> Self {
    self.row_spacing = spacing;
    self
  }

  pub fn get_rows(&self) -> Vec<&Row<'a>> {
    self.header.as_ref().into_iter().chain(self.rows.iter()).collect()
  }

  fn get_column_areas(&self, area: Rect, has_selection: bool) -> Vec<Rect> {
    Self::calculate_column_areas(
      area,
      has_selection,
      self.widths,
      self.column_spacing,
      self.highlight_symbol.map(String::from),
    )
  }
  pub fn calculate_column_areas(
    area: Rect,
    has_selection: bool,
    widths: &[Constraint],
    column_spacing: u16,
    highlight_symbol: Option<String>,
  ) -> Vec<Rect> {
    let mut constraints = Vec::with_capacity(widths.len() * 2 + 1);
    if has_selection {
      let highlight_symbol_width = highlight_symbol.map(|s| s.width() as u16).unwrap_or(0);
      constraints.push(Constraint::Length(highlight_symbol_width));
    }
    for constraint in widths {
      constraints.push(*constraint);
      constraints.push(Constraint::Length(column_spacing));
    }
    if !widths.is_empty() {
      constraints.pop();
    }
    let mut chunks = tui::layout::Layout::default()
      .direction(tui::layout::Direction::Horizontal)
      .constraints(constraints)
      .split(area);
    if has_selection {
      chunks.remove(0);
    }
    chunks.into_iter().step_by(2).collect()
  }

  pub fn row_heights(&self) -> Vec<u16> {
    self.rows.iter().map(|r| r.total_height() + self.row_spacing).collect()
  }

  fn get_row_extents(
    &self,
    // selected: Option<usize>,
    vertical_scroll: u16,
    row_spacing: u16,
    // offset: usize, // first message index to display
    max_height: u16,
  ) -> Vec<Option<(usize, u16, u16, u16)>> {
    // example scroll mechanics diagram
    // row_index         row_line                 line_offset_index
    // 0  start          1   S -----------------------         -4
    // 1                 2       row_skip_lines: 4           -3
    // 2                 3                                   -2
    // 3                 4                                   -1 vertical_scroll = 4
    // 4                 5                                    0 [table_start_index = 4]
    // 5                 6   E -----------------------        1 TABLE AREA
    // 6                 7                                    2 TABLE AREA
    // 7                 8   S -----------------------        3 TABLE AREA
    // 8  srt+end        9                                    4 TABLE AREA
    // 9                 10                                   5 TABLE AREA
    // 10 srt idx        11                                   6 TABLE AREA
    // 11                12                                   7 TABLE_AREA
    // 12 end idx        13  <-- row max                      8 [table end index = 12]
    // 13                   E-----------------------          9 max_height = 9
    // 14                8
    // 15                1  S -----------------------
    // 16 end idx        2
    // 17                3 E -----------------------
    //                   5
    //
    let mut row_index = 0;

    self
    .rows
      .iter()
      // .skip(offset)
      .enumerate()
      .map(|(i, row_text)| {
          let row_start_index = row_index;
          let row_end_index = (row_start_index + row_text.height).saturating_sub(1);

          let table_start_index = vertical_scroll;
          let table_end_index = (vertical_scroll + max_height).saturating_sub(  1);

          row_index += row_text.height + row_spacing;

          let row_skip_lines = if row_start_index < table_start_index {
            table_start_index.saturating_sub(row_start_index)
          } else {
              0
          };

          let row_y =  row_start_index.saturating_sub(table_start_index).saturating_sub(row_skip_lines);

          let row_visible_lines =
              (row_end_index + 1).min(table_end_index).saturating_sub(row_start_index).saturating_sub(row_skip_lines);

        // if i< 3{
        //   log::info!("
        //       row index: {}
        //       row_text.height: {}
        //       row_y: {}
        //       row_skip_lines: {}
        //       row_visible_lines: {}
        //       row_start_index: {}
        //       row_end_index: {}
        //       table_start_index: {}
        //       max_height: {}
        //       row_text: {:?}",
        //       i,
        //     row_text.height,
        //     row_y,
        //     row_skip_lines,
        //     row_visible_lines,
        //       row_start_index,
        //       row_end_index,
        //       table_start_index,
        //       max_height,
        //       String::from(&row_text.cells[1].content)
        //       );
        //     };

        if row_end_index < table_start_index ||
        row_start_index > table_end_index {
            None
        } else {
            Some((i, row_y, row_skip_lines , row_visible_lines ))
        }
      })
    .collect::<Vec<Option<(usize, u16, u16, u16)>>>()
  }
}

#[derive(Debug, Default, Clone)]
pub struct TableState {
  pub scroll_offset: u16,
  pub vertical_scroll: u16,
  pub scroll_max: u16,
  pub row_heights: Vec<u16>,
  pub sticky_scroll: bool,
  pub viewport_height: u16,
  pub selected: Option<usize>,
  pub cursor_position: Option<Position>,
  pub cursor_style: Option<Style>,
  pub select_range: Option<Range>,
}

impl TableState {
  // if the scroll is at the end, scroll with incoming text
  pub fn update_sticky_scroll(&mut self) {
    self.scroll_max = self.row_heights.iter().sum::<u16>().saturating_sub(self.viewport_height);
    // .saturating_sub(self.viewport_height.saturating_sub(self.scroll_offset));

    // .saturating_sub(self.viewport_height.saturating_sub(0));
    if self.sticky_scroll {
      self.vertical_scroll = self.scroll_max;
    }
  }

  pub fn scroll_top(&mut self) {
    self.sticky_scroll = false;
    self.vertical_scroll = 0;
  }
  pub fn scroll_by(&mut self, amount: u16, direction: Direction) {
    match direction {
      Direction::Forward => {
        self.vertical_scroll =
          self.vertical_scroll.saturating_add(amount).clamp(0, self.scroll_max);
      },
      Direction::Backward => {
        self.sticky_scroll = false;
        self.vertical_scroll =
          self.vertical_scroll.saturating_sub(amount).clamp(0, self.scroll_max);
      },
    }

    if self.vertical_scroll == self.scroll_max {
      self.sticky_scroll = true;
    }
  }

  pub fn scroll_to_selection(&mut self) {
    if let Some(selected) = self.selected {
      let selection_top: u16 = self.row_heights.iter().take(selected).sum();
      let selection_bottom: u16 = self.row_heights.iter().take(selected + 1).sum();

      if selection_bottom > self.vertical_scroll + self.viewport_height {
        self.vertical_scroll = selection_bottom.saturating_sub(self.viewport_height);
      }

      if selection_top < self.vertical_scroll {
        self.vertical_scroll = selection_top
      }
    }
  }

  pub fn selected(&self) -> Option<usize> {
    self.selected
  }

  pub fn select(&mut self, index: Option<usize>) {
    self.selected = index;
    if index.is_none() {
      self.scroll_offset = 0;
    }
  }
}

// impl<'a> StatefulWidget for Table<'a> {
impl<'a> Table<'a> {
  // type State = TableState;

  pub fn render_table(
    mut self,
    area: Rect,
    buf: &mut Buffer,
    state: &mut TableState,
    truncate: bool,
    theme: &Theme,
    config_loader: &Arc<ArcSwap<syntax::Loader>>,
  ) {
    buf.set_style(area, self.style);
    state.viewport_height = area.height;

    state.update_sticky_scroll();
    let table_area = match self.block.take() {
      Some(b) => {
        let inner_area = b.inner(area);
        b.render(area, buf);
        inner_area
      },
      None => area,
    };

    // self.rows.iter().enumerate().for_each(|(i, row)| {
    // log::warn!(
    //   "row idx: {}  cell count: {}",
    //   i,
    //   row.cells.len(),
    //   // row.cell_text().collect::<String>()
    // );
    // });
    let has_selection = state.selected.is_some();
    let column_areas = self.get_column_areas(table_area, has_selection);
    let column_widths = column_areas.iter().map(|a| a.width).collect::<Vec<_>>();
    self.rows.iter_mut().for_each(|row| row.update_wrapped_heights(column_widths.clone()));

    state.row_heights = self.row_heights();

    let highlight_symbol = self.highlight_symbol.unwrap_or("");
    let blank_symbol = " ".repeat(highlight_symbol.width());
    let mut current_height = 0;

    // Draw header
    if let Some(ref header) = self.header {
      let max_header_height = table_area.height.min(header.total_height());
      buf.set_style(
        Rect {
          x: table_area.left(),
          y: table_area.top(),
          width: table_area.width,
          height: table_area.height.min(header.height),
        },
        header.style,
      );
      let mut col = table_area.left();
      if has_selection {
        col += (highlight_symbol.width() as u16).min(table_area.width);
      }
      for (width, cell) in column_widths.iter().zip(header.cells.iter()) {
        render_cell(
          buf,
          cell,
          Rect { x: col, y: table_area.top(), width: *width, height: max_header_height },
          0u16,
          truncate,
          theme,
          config_loader,
        );
        col += *width + self.column_spacing;
      }
      current_height += max_header_height;
    }

    // Draw rows
    if self.rows.is_empty() {
      return;
    }

    for (table_row, extents) in self.rows.iter().zip(self.get_row_extents(
      state.vertical_scroll,
      self.row_spacing,
      table_area.height - current_height,
    )) {
      match extents {
        None => continue,
        Some((i, row_y, row_skip_lines, row_height)) => {
          // log::info!(
          //   "rendering row:\nrow idx: {}  row_y: {}  row_height: {} skip: {}",
          //   i,
          //   row_y,
          //   row_height,
          //   row_skip_lines
          // );
          if row_height == 0 {
            continue;
          }
          let (row, col) = (table_area.top() + row_y, table_area.left());
          // row_height.min(table_area.height.saturating_sub(current_height));
          let table_row_area = Rect { x: col, y: row, width: table_area.width, height: row_height };

          // buf.set_style(table_row_area, table_row.style);
          let is_selected = state.selected.map(|s| s == i).unwrap_or(false);
          let table_row_start_col = if has_selection {
            let symbol = if is_selected { highlight_symbol } else { &blank_symbol };
            let (col, _) =
              buf.set_stringn(col, row, symbol, table_area.width as usize, table_row.style);
            col
          } else {
            col
          };
          if is_selected {
            buf.set_style(table_row_area, self.highlight_style);
          }
          let mut col = table_row_start_col;
          for (width, cell) in column_widths.iter().zip(table_row.cells.iter()) {
            // log::debug!(
            //   "rendering cell - width: {}  col: {}  row: {}  height: {}",
            //   width,
            //   col,
            //   row,
            //   table_row_area.height
            // );
            render_cell(
              buf,
              cell,
              Rect { x: col, y: table_row_area.y, height: table_row_area.height, width: *width },
              row_skip_lines,
              truncate,
              theme,
              config_loader,
            );
            col += *width + self.column_spacing;
          }
        },
      }
    }
  }
}

fn render_cell(
  buf: &mut Buffer,
  cell: &MessageCell,
  area: Rect,
  skip_lines: u16,
  _truncate: bool,
  theme: &Theme,
  config_loader: &Arc<ArcSwap<syntax::Loader>>,
) {
  // log::warn!(
  //   "rendering cell x: {}  y: {} height:{}",
  //   area.x,
  //   area.y,
  //   area.height
  // );

  cell.render_cell(buf, area, theme, skip_lines, config_loader);
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  #[should_panic]
  fn table_invalid_percentages() {
    Table::new(vec![]).widths(&[Constraint::Percentage(110)]);
  }
}
