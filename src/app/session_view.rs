use bat::{assets::HighlightingAssets, config::Config, controller::Controller, style::StyleComponents, Input};

use color_eyre::owo_colors::OwoColorize;
use textwrap::{self, Options, WordSeparator, WordSplitter, WrapAlgorithm};

use crate::trace_dbg;

use super::{messages::MessageContainer, session_data::SessionData};
use ropey::Rope;

#[derive(Default, Debug)]
pub struct SessionView {
  pub renderer: BatRenderer<'static>,
  pub window_width: usize,
  pub render_conditions: (usize, usize, usize, usize, bool),
  pub rendered_view: String,
  pub new_data: bool,
  pub rendered_text: Rope,
}

impl SessionView {
  pub fn set_window_width(&mut self, width: usize, _messages: &mut [MessageContainer]) {
    let new_value = width - 6;
    if self.window_width != new_value {
      trace_dbg!("setting window width to {}", new_value);

      self.window_width = new_value;
      self.renderer.config.term_width = 10000;
      //self.renderer.config.term_width = new_value;
    }
  }

  pub fn get_stylized_rendered_slice(&mut self, start_line: usize, line_count: usize, vertical_scroll: usize) -> &str {
    if (start_line, line_count, vertical_scroll, self.rendered_text.len_chars(), self.new_data)
      != self.render_conditions
    {
      self.render_conditions = (start_line, line_count, vertical_scroll, self.rendered_text.len_chars(), self.new_data);
      //   trace_dbg!(
      //   "get_stylized_rendered_slice: start_line: {}, line_count: {}, vertical_scroll: {}, rendered_text.len_lines(): {}",
      //   start_line,
      //   line_count,
      //   vertical_scroll,
      //   self.rendered_text.len_lines()
      // );
      self.new_data = false;
      // let text =
      // self.rendered_text.lines().skip(start_line).take(line_count).map(|c| c.to_string()).collect::<String>();
      // let wrapped_text = bwrap::wrap!(&text, self.window_width);
      //let rendered_text = &self.renderer.render_message_bat(start_line, line_count, &self.rendered_text);
      let rendered_text =
        &self.rendered_text.lines_at(start_line).take(line_count + 1).map(|l| l.to_string()).collect::<String>();
      //let debug = format!("{}\n", rendered_text);
      //trace_dbg!(debug);
      self.rendered_view = "-\n".repeat(vertical_scroll) + rendered_text.as_str()
    }
    &self.rendered_view
  }

  pub fn post_process_new_messages(&mut self, session_data: &mut SessionData) {
    let dividing_newlines_count = 2;
    session_data.messages.iter_mut().for_each(|message| {
      let rendered_text_message_start_index = self.rendered_text.len_chars() - message.stylized.len_chars();
      // trace_dbg!("message: {:#?}", message.bright_blue());
      // let previously_rendered_bytecount = message.rendered.stylized.len_bytes();
      if !message.stylize_complete {
        let text_width = self.window_width.min(80);
        let left_padding = self.window_width.saturating_sub(text_width) / 2;
        trace_dbg!("left_padding: {}\ttext_width: {}, window_width: {}", left_padding, text_width, self.window_width);
        let stylized = self.renderer.render_message_bat(format!("{}", &message).as_str());
        let options = Options::new(text_width)
          //.break_words(false)
          .word_splitter(WordSplitter::NoHyphenation)
          .word_separator(WordSeparator::AsciiSpace)
        .wrap_algorithm(WrapAlgorithm::new_optimal_fit());
        let wrapped = textwrap::wrap(stylized.as_str(), options);

        message.stylized = Rope::from_str(
          wrapped
            .iter()
            .enumerate()
            .map(|(i, l)| {
              if i == 0 {
                format!("{}{}", " ".repeat(left_padding + 2), l)
              } else {
                format!("{}{}", " ".repeat(left_padding + 4), l)
              }
            })
            .collect::<Vec<String>>()
            .join("\n")
            .as_str(),
        );
        //message.rendered.stylized = Rope::from_str(&message.rendered.content);
        if message.receive_complete {
          message.stylized.append(Rope::from_str("\n".to_string().repeat(dividing_newlines_count).as_str()));
          message.stylize_complete = true;
        }

        self.new_data = true;
        self.rendered_text.remove(rendered_text_message_start_index..);
        self.rendered_text.append(message.stylized.clone());
        // message.rendered.stylized.bytes_at(previously_rendered_bytecount).for_each(|b| {
        //   self.renderer.rendered_bytes.push(b);
        // });
        // trace_dbg!("bytes: {:?}", self.renderer.rendered_bytes.len());
      }
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
      language: Some("markdown"),
      style_components,
      show_nonprintable: false,
      tab_width: 2,
      wrapping_mode: bat::WrappingMode::NoWrapping(false),
      use_italic_text: true,
      term_width,
      paging_mode: bat::PagingMode::Never,
      true_color: true,
      use_custom_assets: true,
      ..Default::default()
    };
    let assets = HighlightingAssets::from_binary();
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
    let input = Input::from_bytes(text.as_bytes());
    //trace_dbg!("render_message_bat: {:?}", self.rendered_bytes);
    controller.run_with_error_handler(vec![input.into()], Some(&mut buffer), Self::render_error).unwrap();
    //trace_dbg!("render_message_bat: {:?}", buffer);
    buffer
  }
}
