use bat::{
  assets::HighlightingAssets, config::Config, controller::Controller,
  style::StyleComponents,
};
use helix_core::config::default_lang_config;
use helix_core::syntax::Loader;
use helix_view::Theme;
use std::default::Default;
use std::sync::Arc;

use crate::trace_dbg;

use super::messages::MessageContainer;
use ropey::Rope;

#[derive(Debug)]
pub struct SessionView {
  pub lang_config: Arc<Loader>,
  pub theme: Theme,
  pub renderer: BatRenderer<'static>,
  pub window_width: usize,
  pub render_conditions: (usize, usize, usize, usize, bool),
  pub rendered_view: String,
  // pub textarea: TextArea<'a>,
  pub selected_text: Option<String>,
  pub new_data: bool,
  pub rendered_text: Rope,
}

impl Default for SessionView {
  fn default() -> Self {
    SessionView {
      lang_config: Arc::new(
        Loader::new(default_lang_config())
          .expect("could not initialize language config"),
      ),
      ..Default::default()
    }
  }
}

impl SessionView {
  pub fn unfocus_textarea(&mut self) {
    // use ratatui::style::{Color, Style};
    // self.textarea.set_cursor_line_style(Style::default());
    // self.textarea.set_cursor_style(Style::default());
    // self.textarea.set_block(
    //   Block::default()
    //     .borders(Borders::ALL)
    //     .style(Style::default().fg(Color::DarkGray))
    //     .title(" Inactive (^X to switch) "),
    // );
  }

  pub fn focus_textarea(&mut self) {
    // use ratatui::style::{Color, Style};
    // self.textarea.move_cursor(CursorMove::Top);
    // self.textarea.move_cursor(CursorMove::Head);
    // self.textarea.set_cursor_line_style(
    //   Style::default()
    //     .add_modifier(Modifier::UNDERLINED)
    //     .add_modifier(Modifier::SLOW_BLINK),
    // );
    // self.textarea.set_cursor_style(Style::default().bg(Color::Yellow));
    // self.textarea.set_block(
    //   Block::default()
    //     .borders(Borders::ALL)
    //     .style(Style::default())
    //     .title(" Active "),
    // );
  }

  pub fn set_window_width(
    &mut self,
    width: usize,
    _messages: &mut [MessageContainer],
  ) {
    let new_value = width;
    if self.window_width != new_value {
      trace_dbg!("setting window width to {}", new_value);

      self.window_width = new_value;
      self.renderer.config.term_width = new_value;
      //self.renderer.config.term_width = new_value;
    }
  }

  pub fn post_process_new_messages(
    &mut self,
    messages: &mut [MessageContainer],
  ) {
    let dividing_newlines_count = 2;
    messages.iter_mut().filter(|m| !m.is_complete()).for_each(|message| {
      // message.render_text(self.window_width, self.lang_config);
      // let rendered_text_message_start_index
      //   self.rendered_text.len_chars() - message.stylized.len_chars();
      // let original_message_length = message.stylized.len_chars();
      // if !message.stylize_complete {
      //   let text_width = self.window_width.min(80 - 4);
      //   let stylized =
      //     self.renderer.render_message_bat(format!("{}", &message).as_str());
      //   let options = Options::new(text_width)
      //     .initial_indent("")
      //     .subsequent_indent("  ")
      //     .word_splitter(WordSplitter::NoHyphenation)
      //     .word_separator(WordSeparator::AsciiSpace)
      //     .wrap_algorithm(WrapAlgorithm::new_optimal_fit());
      //   let wrapped = textwrap::wrap(stylized.as_str(), options);
      //
      //   message.stylized = Rope::from_str(
      //     wrapped
      //       .iter()
      //       .map(|c| c.to_string())
      //       .collect::<Vec<String>>()
      //       .join("\n")
      //       .as_str(),
      //   );
      //
      //   if message.receive_complete {
      //     message.stylized.append(Rope::from_str(
      //       "\n".to_string().repeat(dividing_newlines_count).as_str(),
      //     ));
      //     message.stylize_complete = true;
      //   }
      //
      //   self.new_data = true;
      //   self.textarea.replace_at_end(
      //     message.stylized.to_string(),
      //     original_message_length,
      //   );
      //
      //   self.rendered_text.remove(rendered_text_message_start_index..);
      //   self.rendered_text.append(message.stylized.clone());
      // }
    });
  }
}

pub struct BatRenderer<'a> {
  assets: HighlightingAssets,
  config: Config<'a>,
}

impl<'a> Default for BatRenderer<'a> {
  fn default() -> Self {
    BatRenderer::new(80)
  }
}

impl<'a> std::fmt::Debug for BatRenderer<'a> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BatRenderer").finish()
  }
}

impl<'a> BatRenderer<'a> {
  fn new(term_width: usize) -> Self {
    let style_components = StyleComponents::new(&[
      //StyleComponent::Header,
      //StyleComponent::Grid,
      //StyleComponent::LineNumbers,
      //StyleComponent::Changes,
      //StyleComponent::Rule,
      //StyleComponent::Default,
      //StyleComponent::Snip,
      //StyleComponents::plain,
    ]);
    let config: Config<'static> = Config {
      colored_output: true,
      language: Some("Markdown"),
      style_components,
      show_nonprintable: false,
      tab_width: 2,
      wrapping_mode: bat::WrappingMode::Character,
      // wrapping_mode: bat::WrappingMode::NoWrapping(false),
      use_italic_text: true,
      term_width,
      paging_mode: bat::PagingMode::Never,
      true_color: true,
      use_custom_assets: false,
      ..Default::default()
    };
    let assets = HighlightingAssets::from_binary();
    // let assets = HighlightingAssets::from_cache(&Path::new("./lib/bat/assets")).unwrap();
    let _buffer: Vec<u8> = Vec::new();
    BatRenderer { config, assets }
  }

  fn render_error(err: &bat::error::Error, _write: &mut dyn std::io::Write) {
    trace_dbg!("bat rendering error: {}", err);
  }

  fn render_message_bat(&mut self, text: &str) -> String {
    //    fn render_message_bat(&self, content: &str) -> String {
    // self.config.visible_lines =
    //   VisibleLines::Ranges(LineRanges::from(vec![LineRange::new(start_line, start_line + line_count)]));

    let controller = Controller::new(&self.config, &self.assets);
    let mut buffer = String::new();
    //let input = Input::from_bytes(content.as_bytes());
    //trace_dbg!("render_message_bat: {:?}", self.rendered_bytes);

    //text.bytes().skip(self.buffer.len()).for_each(|b| self.buffer.push(b));
    let input = bat::Input::from_bytes(text.as_bytes());
    //trace_dbg!("render_message_bat: {:?}", self.rendered_bytes);
    controller
      .run_with_error_handler(
        vec![input.into()],
        Some(&mut buffer),
        Self::render_error,
      )
      .unwrap();
    //trace_dbg!("render_message_bat: {:?}", buffer);
    buffer
  }
}

// fn visual_char_coord_to_index(
//   rope: &Rope,
//   line_number: usize,
//   x_coord: usize,
// ) -> usize {
//   // Clamp line_number within the number of lines in rope
//   let line_number = line_number.min(rope.len_lines());
//   // Get the character offset of the line
//   let line_offset = rope.line_to_char(line_number);
//   // Get the line as a slice
//   let line = rope.line(line_number);
//   // Clamp x_coord within the number of characters in line
//   let x_coord = x_coord.min(line.len_chars());
//
//   let mut visual_char_index = 0;
//   let mut actual_char_index = 0;
//
//   // Iterate over the characters of the line
//   while actual_char_index < line.len_chars() {
//     let c = line.char(actual_char_index);
//
//     if c == '\\' {
//       // Detect ASCII escape sequences
//       if actual_char_index + 1 < line.len_chars() {
//         let next_c = line.char(actual_char_index + 1);
//         if matches!(next_c, 'n' | 'r' | 't' | '\\' | '0') {
//           actual_char_index += 2; // Skip the escape sequence
//           continue;
//         }
//       }
//     } else if c == '\x1B' {
//       // Detect ANSI escape codes
//       if actual_char_index + 1 < line.len_chars()
//         && line.char(actual_char_index + 1) == '['
//       {
//         actual_char_index += 2; // Skip the escape character and '['
//                                 // Loop until we find a character that is not part of the code
//         while actual_char_index < line.len_chars()
//           && !line.char(actual_char_index).is_alphabetic()
//         {
//           actual_char_index += 1;
//         }
//         actual_char_index += 1; // Skip the final character of the ANSI code
//         continue;
//       }
//     }
//
//     // If we've reached the x_coord, return the index
//     if visual_char_index == x_coord {
//       return line_offset + actual_char_index;
//     }
//
//     visual_char_index += 1;
//     actual_char_index += 1;
//   }
//
//   // If x_coord was not found, return the end of the line offset
//   line_offset + line.len_chars()
// }
