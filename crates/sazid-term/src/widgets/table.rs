use helix_core::unicode::width::UnicodeWidthStr;
use helix_view::graphics::{Rect, Style};
use tui::{
  buffer::Buffer,
  layout::{Alignment, Constraint},
  text::Text,
  widgets::{Block, Widget, Wrap},
};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParagraphCell<'a> {
  /// A block to wrap the widget in
  block: Option<Block<'a>>,
  /// How to wrap the text
  wrap_trim: Option<bool>,
  /// Scroll
  scroll: (u16, u16),
  /// Alignment of the text
  alignment: Alignment,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cell<'a> {
  /// The text to display
  pub content: Text<'a>,
  /// Widget style
  style: Style,
  //// Paragraph options for multi line cells
  pub paragraph_options: Option<ParagraphCell<'a>>,
}

impl<'a> Cell<'a> {
  /// Set the `Style` of this cell.
  pub fn style(mut self, style: Style) -> Self {
    self.style = style;
    self
  }

  pub fn paragraph_cell(
    mut self,
    block: Option<Block<'a>>,
    wrap_trim: Option<bool>,
    scroll: (u16, u16),
    alignment: Alignment,
  ) -> Self {
    self.paragraph_options =
      Some(ParagraphCell { block, wrap_trim, scroll, alignment });
    self
  }
}

impl<'a, T> From<T> for Cell<'a>
where
  T: Into<Text<'a>>,
{
  fn from(content: T) -> Cell<'a> {
    Cell {
      content: content.into(),
      style: Style::default(),
      paragraph_options: None,
    }
  }
}

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
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Row<'a> {
  pub cells: Vec<Cell<'a>>,
  height: u16,
  style: Style,
  bottom_margin: u16,
}

impl<'a> Row<'a> {
  /// Creates a new [`Row`] from an iterator where items can be converted to a [`Cell`].
  pub fn new<T>(cells: T) -> Self
  where
    T: IntoIterator,
    T::Item: Into<Cell<'a>>,
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
    self.cells.iter().map(|cell| String::from(&cell.content))
  }
}

impl<'a, T: Into<Cell<'a>>> From<T> for Row<'a> {
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
      row_spacing: 0,
      highlight_style: Style::default(),
      highlight_symbol: None,
      header: None,
      rows: rows.into_iter().collect(),
    }
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

  fn get_columns_widths(
    &self,
    max_width: u16,
    has_selection: bool,
  ) -> Vec<u16> {
    let mut constraints = Vec::with_capacity(self.widths.len() * 2 + 1);
    if has_selection {
      let highlight_symbol_width =
        self.highlight_symbol.map(|s| s.width() as u16).unwrap_or(0);
      constraints.push(Constraint::Length(highlight_symbol_width));
    }
    for constraint in self.widths {
      constraints.push(*constraint);
      constraints.push(Constraint::Length(self.column_spacing));
    }
    if !self.widths.is_empty() {
      constraints.pop();
    }
    let mut chunks = tui::layout::Layout::default()
      .direction(tui::layout::Direction::Horizontal)
      .constraints(constraints)
      .split(Rect { x: 0, y: 0, width: max_width, height: 1 });
    if has_selection {
      chunks.remove(0);
    }
    chunks.iter().step_by(2).map(|c| c.width).collect()
  }

  pub fn row_heights(&self) -> Vec<u16> {
    self.rows.iter().map(|r| r.total_height()).collect()
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

          let row_y =  row_start_index.saturating_sub(table_start_index).saturating_sub(row_skip_lines) ;

          let row_visible_lines =
              row_end_index.min(table_end_index).saturating_sub(row_start_index).saturating_sub(row_skip_lines);

        if i< 3{
          log::info!("
              row index: {}
              row_text.height: {}
              row_y: {}
              row_skip_lines: {}
              row_visible_lines: {}
              row_start_index: {}
              row_end_index: {}
              table_start_index: {}
              max_height: {}",
              i,
            row_text.height,
            row_y,
            row_skip_lines,
            row_visible_lines,
              row_start_index,
              row_end_index,
              table_start_index,
              max_height, );
            };

        if row_end_index < table_start_index {
            None
        } else if row_start_index > table_end_index {
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
  pub offset: usize,
  pub vertical_scroll: u32,
  pub selection_heights: Vec<u16>,
  pub viewport_height: u32,
  pub selected: Option<usize>,
}

impl TableState {
  pub fn scroll_to_selection(&mut self) {
    if let Some(selected) = self.selected {
      let selection_top: u32 =
        self.selection_heights.iter().take(selected).sum::<u16>().into();

      let selection_bottom: u32 =
        self.selection_heights.iter().take(selected + 1).sum::<u16>().into();

      log::info!(
        "scroll to selection: {}-{}, {} {} {} {}",
        self.vertical_scroll,
        self.vertical_scroll + self.viewport_height,
        self.viewport_height,
        self.selection_heights[selected],
        selection_top,
        selection_bottom
      );

      if selection_bottom > self.vertical_scroll + self.viewport_height {
        self.vertical_scroll =
          selection_bottom.saturating_sub(self.viewport_height);
      }

      if selection_top < self.vertical_scroll {
        self.vertical_scroll = selection_top
      }
    }
    log::info!(
      "scroll to selection: {} {:?}",
      self.vertical_scroll,
      self.selected
    );
  }

  pub fn selected(&self) -> Option<usize> {
    self.selected
  }

  pub fn select(&mut self, index: Option<usize>) {
    self.selected = index;
    if index.is_none() {
      self.offset = 0;
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
  ) {
    if area.area() == 0 {
      return;
    }
    buf.set_style(area, self.style);
    state.selection_heights = self.row_heights();
    state.viewport_height = area.height as u32;
    let table_area = match self.block.take() {
      Some(b) => {
        let inner_area = b.inner(area);
        b.render(area, buf);
        inner_area
      },
      None => area,
    };

    let has_selection = state.selected.is_some();
    let columns_widths =
      self.get_columns_widths(table_area.width, has_selection);
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
      for (width, cell) in columns_widths.iter().zip(header.cells.iter()) {
        render_cell(
          buf,
          cell,
          Rect {
            x: col,
            y: table_area.top(),
            width: *width,
            height: max_header_height,
          },
          0u16,
          truncate,
        );
        col += *width + self.column_spacing;
      }
      current_height += max_header_height;
    }

    // Draw rows
    if self.rows.is_empty() {
      return;
    }

    // let (start, end, start_line, height_truncate_end) = self
    //   .get_row_bounds(
    //     state.selected,
    //     // state.offset,
    //     state.vertical_scroll_lines as u16,
    //     rows_height,
    //   );

    // log::debug!(
    //   "row bounds: {}-{}
    //   selected: {:?}
    //   offset: {}
    //   rows_height: {}
    //   table_area: {:?} ",
    //   start,
    //   end,
    //   state.selected,
    //   state.offset,
    //   rows_height,
    //   table_area
    // );

    // state.offset = start;
    log::debug!(
      "Vertical scroll: {}\nTable area: {:#?}\ncurrent_height: {}",
      state.vertical_scroll,
      table_area,
      current_height
    );

    for (table_row, extents) in self.rows.iter().zip(self.get_row_extents(
      state.vertical_scroll as u16,
      self.row_spacing,
      table_area.height - current_height,
    )) {
      match extents {
        None => continue,
        Some((i, row_y, row_skip_lines, row_height)) => {
          if row_height == 0 {
            continue;
          }
          let (row, col) = (table_area.top() + row_y, table_area.left());
          // row_height.min(table_area.height.saturating_sub(current_height));
          let table_row_area = Rect {
            x: col,
            y: row,
            width: table_area.width,
            height: row_height,
          };
          // log::info!(
          //   "row index: {}
          //       table_row_area: {:#?}
          //       current_height: {},
          //       row_visible_lines: {}
          //       skip_lines: {}
          //       ",
          //   i,
          //   table_row_area,
          //   current_height,
          //   row_height,
          //   row_skip_lines
          // );

          buf.set_style(table_row_area, table_row.style);
          let is_selected = state.selected.map(|s| s == i).unwrap_or(false);
          let table_row_start_col = if has_selection {
            let symbol =
              if is_selected { highlight_symbol } else { &blank_symbol };
            let (col, _) = buf.set_stringn(
              col,
              row,
              symbol,
              table_area.width as usize,
              table_row.style,
            );
            col
          } else {
            col
          };
          if is_selected {
            buf.set_style(table_row_area, self.highlight_style);
          }
          let mut col = table_row_start_col;
          for (width, cell) in columns_widths.iter().zip(table_row.cells.iter())
          {
            render_cell(
              buf,
              cell,
              Rect {
                x: col,
                y: table_row_area.y,
                height: table_row_area.height,
                width: *width,
              },
              // Rect { x: col, y: row, width: *width, height: table_row.height },
              row_skip_lines,
              truncate,
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
  cell: &Cell,
  area: Rect,
  skip_lines: u16,
  truncate: bool,
) {
  match cell.paragraph_options.clone() {
    Some(options) => {
      let mut paragraph = tui::widgets::Paragraph::new(&cell.content);
      if let Some(block) = options.block {
        paragraph = paragraph.block(block);
      }
      if let Some(wrap) = options.wrap_trim {
        paragraph = paragraph.wrap(Wrap { trim: wrap });
      }
      paragraph = paragraph.scroll((skip_lines, 0));
      paragraph = paragraph.alignment(options.alignment);
      paragraph.render(area, buf);
    },
    None => {
      buf.set_style(area, cell.style);
      for (i, spans) in
        cell.content.lines.iter().skip(skip_lines.into()).enumerate()
      {
        if i as u16 >= area.height {
          break;
        }
        if truncate {
          buf.set_spans_truncated(area.x, area.y + i as u16, spans, area.width);
        } else {
          buf.set_spans(area.x, area.y + i as u16, spans, area.width);
        }
      }
    },
  }
}

impl<'a> Widget for Table<'a> {
  fn render(self, area: Rect, buf: &mut Buffer) {
    let mut state = TableState::default();
    Table::render_table(self, area, buf, &mut state, false);
  }
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
