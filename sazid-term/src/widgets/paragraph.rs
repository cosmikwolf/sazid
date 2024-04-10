use super::reflow::{LineComposer, LineTruncator, WordWrapper};
use helix_core::{
  syntax::{Highlight, HighlightEvent},
  unicode::width::UnicodeWidthStr,
  RopeSlice,
};
use helix_lsp::{lsp::Range, Position};
use helix_view::{
  graphics::{Rect, Style},
  theme::Color,
  Theme,
};
use std::iter;
use tui::{
  buffer::{Buffer, Cell},
  layout::Alignment,
  text::{StyledGrapheme, Text},
  widgets::{Block, Widget},
};

fn get_line_offset(line_width: u16, text_area_width: u16, alignment: Alignment) -> u16 {
  match alignment {
    Alignment::Center => (text_area_width / 2).saturating_sub(line_width / 2),
    Alignment::Right => text_area_width.saturating_sub(line_width),
    Alignment::Left => 0,
  }
}

/// A widget to display some text.
///
/// # Examples
///
/// ```
/// # use helix_tui::text::{Text, Spans, Span};
/// # use helix_tui::widgets::{Block, Borders, Paragraph, Wrap};
/// # use helix_tui::layout::{Alignment};
/// # use helix_view::graphics::{Style, Color, Modifier};
/// let text = Text::from(vec![
///     Spans::from(vec![
///         Span::raw("First"),
///         Span::styled("line",Style::default().add_modifier(Modifier::ITALIC)),
///         Span::raw("."),
///     ]),
///     Spans::from(Span::styled("Second line", Style::default().fg(Color::Red))),
/// ]);
/// Paragraph::new(&text)
///     .block(Block::default().title("Paragraph").borders(Borders::ALL))
///     .style(Style::default().fg(Color::White).bg(Color::Black))
///     .alignment(Alignment::Center)
///     .wrap(Wrap { trim: true });
/// ```
#[derive(Debug, Clone)]
pub struct Paragraph<'a> {
  /// A block to wrap the widget in
  block: Option<Block<'a>>,
  /// Widget style
  style: Style,
  /// Highlight style
  highlight_style: Option<Style>,
  /// Highlight Range
  highlight_range: Option<std::ops::Range<usize>>,
  /// How to wrap the text
  wrap: Option<Wrap>,
  /// The text to display
  text: &'a Text<'a>,
  /// Scroll
  scroll: (u16, u16),
  /// Alignment of the text
  alignment: Alignment,
  /// offset of char in full text of table
  char_idx: Option<usize>,
}

/// Describes how to wrap text across lines.
///
/// ## Examples
///
/// ```
/// # use helix_tui::widgets::{Paragraph, Wrap};
/// # use helix_tui::text::Text;
/// let bullet_points = Text::from(r#"Some indented points:
///     - First thing goes here and is long so that it wraps
///     - Here is another point that is long enough to wrap"#);
///
/// // With leading spaces trimmed (window width of 30 chars):
/// Paragraph::new(&bullet_points).wrap(Wrap { trim: true });
/// // Some indented points:
/// // - First thing goes here and is
/// // long so that it wraps
/// // - Here is another point that
/// // is long enough to wrap
///
/// // But without trimming, indentation is preserved:
/// Paragraph::new(&bullet_points).wrap(Wrap { trim: false });
/// // Some indented points:
/// //     - First thing goes here
/// // and is long so that it wraps
/// //     - Here is another point
/// // that is long enough to wrap
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Wrap {
  /// Should leading whitespace be trimmed
  pub trim: bool,
}

impl<'a> Paragraph<'a> {
  pub fn new(text: &'a Text) -> Paragraph<'a> {
    Paragraph {
      block: None,
      style: Default::default(),
      wrap: None,
      highlight_style: None,
      highlight_range: None,
      text,
      scroll: (0, 0),
      char_idx: None,
      alignment: Alignment::Left,
    }
  }

  pub fn set_highlight_options(
    mut self,
    highlight_style: Option<Style>,
    highlight_range: Option<std::ops::Range<usize>>,
  ) -> Paragraph<'a> {
    self.highlight_style = highlight_style;
    self.highlight_range = highlight_range;
    self
  }

  pub fn block(mut self, block: Block<'a>) -> Paragraph<'a> {
    self.block = Some(block);
    self
  }

  pub fn style(mut self, style: Style) -> Paragraph<'a> {
    self.style = style;
    self
  }

  pub fn wrap(mut self, wrap: Wrap) -> Paragraph<'a> {
    self.wrap = Some(wrap);
    self
  }

  pub fn char_idx(mut self, char_idx: usize) -> Paragraph<'a> {
    self.char_idx = Some(char_idx);
    self
  }
  pub fn scroll(mut self, offset: (u16, u16)) -> Paragraph<'a> {
    self.scroll = offset;
    self
  }

  pub fn alignment(mut self, alignment: Alignment) -> Paragraph<'a> {
    self.alignment = alignment;
    self
  }

  pub fn wrapped_line_count(&self, width: u16) -> usize {
    let mut styled = self.text.lines.iter().flat_map(|spans| {
      spans
        .0
        .iter()
        .flat_map(|span| span.styled_graphemes(self.style))
        .chain(iter::once(StyledGrapheme { symbol: "\n", style: self.style }))
    });

    let mut line_composer: Box<dyn LineComposer> = if let Some(Wrap { trim }) = self.wrap {
      Box::new(WordWrapper::new(&mut styled, width, trim))
    } else {
      Box::new(LineTruncator::new(&mut styled, width))
    };

    let mut line_count = 0;
    while line_composer.next_line().is_some() {
      line_count += 1;
    }

    line_count
  }
}

#[allow(clippy::too_many_arguments)]
pub fn format_text(
  text: &Text<'_>,
  style: Style,
  wrap: Option<Wrap>,
  area: Rect,
  alignment: Alignment,
  scroll: (u16, u16),
  char_idx: Option<usize>,
  highlight_range: Option<std::ops::Range<usize>>,
  highlight_style: Option<Style>,
) -> Buffer {
  log::warn!("format text y: {} height:{}", area.y, area.height);
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
    let mut line_composer = Box::new(LineTruncator::new(&mut styled, area.width));
    if alignment == Alignment::Left {
      line_composer.set_horizontal_offset(scroll.1);
    }
    line_composer
  };
  let mut y = 0;
  let mut buf = Buffer::empty(area);
  let mut char_counter = char_idx.unwrap();
  let mut last_grapheme_idx = 0;
  while let Some((current_line, current_line_width)) = line_composer.next_line() {
    if y >= scroll.0 {
      let mut x = 0; // get_line_offset(current_line_width, area.width, alignment);
      let mut linetxt = String::new();
      let idx_start = char_counter;
      for (StyledGrapheme { symbol, style }, grapheme_index) in current_line.iter().zip(idx_start..)
      {
        let style = if let (Some(highlight_range), Some(highlight_style)) =
          (highlight_range.as_ref(), highlight_style)
        {
          if highlight_range.contains(&grapheme_index) {
            linetxt.push_str(symbol);
            // log::info!(
            //   "hl: {} {} {} {} {}",
            //   symbol,
            //   area.left() + x,
            //   area.top() + y,
            //   char_counter,
            //   grapheme_index
            // );
            // buf.set_style(
            //   Rect {
            //     x: area.left() + x,
            //     y: area.top() + y - self.scroll.0,
            //     width: symbol.width() as u16,
            //     height: 1,
            //   },
            //   highlight_style,
            // );
            highlight_style
          } else {
            *style
          }
        } else {
          *style
        };
        if symbol.is_empty() {
          // If the symbol is empty, the last char which rendered last time will
          // leave on the line. It's a quick fix.
          " "
        } else {
          symbol
        };
        let cell = &mut buf[(area.left() + x, area.top() + y - scroll.0)];
        cell.set_symbol(symbol).set_style(style);
        x += symbol.width() as u16;
        // char_counter += symbol.width();
        last_grapheme_idx = grapheme_index;
      }
      char_counter += current_line_width as usize;
      // if let Some(ref range) = highlight_range {
      //   log::warn!(
      //   "format_text: {:?}\nlen: {}, x: {} char_counter: {} char_idx: {} range: {:?}",
      //   string_text,
      //   string_text.len(),
      //   x,
      //   char_counter,
      //   char_idx,
      //   range
      // );
      // }
      log::info!(
        "idx: {}   x: {}, y: {}\nsymbol: {}  ",
        char_counter,
        area.left(),
        area.top(),
        linetxt,
      );
    }
    log::error!(
    "text spans sum: {}  plaintext sum: {} spans text sum:{}\n char_idx: {:?}   char_count:{:?}  last_grapheme_idx: {:?}",
      text.lines.iter().map(|s|s.width()).sum::<usize>(),
      String::from(text).len(),
      text.lines.iter().map(String::from).collect::<String>().len(),
      char_idx,
      char_counter,
      last_grapheme_idx
    );
    y += 1;
    if y >= area.height + scroll.0 {
      break;
    }
  }
  buf
}

impl<'a> Paragraph<'a> {
  pub fn render_paragraph(mut self, area: Rect, buf: &mut Buffer) {
    log::warn!("rendering paragraph x: {}  y: {} height:{}", area.x, area.y, area.height);
    let text_area = match self.block.take() {
      Some(b) => {
        let inner_area = b.inner(area);
        b.render(area, buf);
        inner_area
      },
      None => area,
    };
    if text_area.height < 1 {
      return;
    }
    let text_buf = format_text(
      self.text,
      self.style,
      self.wrap,
      text_area,
      self.alignment,
      self.scroll,
      self.char_idx,
      self.highlight_range,
      self.highlight_style,
    );
    buf.merge(&text_buf);
    buf.set_style(area, self.style);
  }
}
