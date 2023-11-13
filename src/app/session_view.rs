use bat::{
  assets::HighlightingAssets,
  config::{Config, VisibleLines},
  controller::Controller,
  line_range::{LineRange, LineRanges},
  style::{StyleComponent, StyleComponents},
  Input,
};

use pulldown_cmark::{Options, Parser};
use pulldown_cmark_mdcat::resources::*;
use pulldown_cmark_mdcat::terminal::{TerminalProgram, TerminalSize};
use pulldown_cmark_mdcat::Settings;
use pulldown_cmark_mdcat::{Environment, Theme};
use std::path::Path;
use std::sync::OnceLock;
use syntect::parsing::SyntaxSet;

use crate::trace_dbg;

use super::{
  messages::{MessageContainer, RenderedChatMessage},
  session_data::SessionData,
};
use ropey::{iter::Chars, Rope, RopeSlice};

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
  pub fn set_window_width(&mut self, width: usize, messages: &mut [MessageContainer]) {
    let new_value = width - 6;
    if self.window_width != new_value {
      trace_dbg!("setting window width to {}", new_value);

      self.window_width = new_value;
      self.renderer.config.term_width = new_value;
      messages.iter_mut().for_each(|m| {
        m.finished = false;
      });
    }
  }

  pub fn get_stylized_rendered_slice(&mut self, start_line: usize, line_count: usize, vertical_scroll: usize) -> &str {
    if (start_line, line_count, vertical_scroll, self.rendered_text.len_chars(), self.new_data)
      != self.render_conditions
    {
      self.render_conditions = (start_line, line_count, vertical_scroll, self.rendered_text.len_chars(), self.new_data);
      trace_dbg!(
      "get_stylized_rendered_slice: start_line: {}, line_count: {}, vertical_scroll: {}, rendered_text.len_lines(): {}",
      start_line,
      line_count,
      vertical_scroll,
      self.rendered_text.len_lines()
    );
      self.new_data = false;
      // let text =
      // self.rendered_text.lines().skip(start_line).take(line_count).map(|c| c.to_string()).collect::<String>();
      // let wrapped_text = bwrap::wrap!(&text, self.window_width);
      let rendered_text = &self.renderer.render_message_bat(start_line, line_count, &self.rendered_text);
      //let rendered_text = &self.rendered_text.to_string();
      let debug = format!("{}\n", rendered_text);
      trace_dbg!(debug);
      self.rendered_view = "-\n".repeat(vertical_scroll) + rendered_text.as_str()
    }
    &self.rendered_view
  }

  pub fn post_process_new_messages(&mut self, session_data: &mut SessionData) {
    let dividing_newlines_count = 2;
    session_data.messages.iter_mut().for_each(|message| {
      let rendered_text_message_start_index = self.rendered_text.len_chars() - message.rendered.stylized.len_chars();

      // let previously_rendered_bytecount = message.rendered.stylized.len_bytes();
      if !message.finished {
        message.rendered = RenderedChatMessage::from(&message.message);
        message.rendered.stylized =
        // Rope::from_str(&message.rendered.content.as_str());
        Rope::from_str(bwrap::wrap!(&message.rendered.content, self.window_width - 3).as_str());

        if message.rendered.finish_reason.is_some() {
          message.finished = true;
          message.rendered.stylized.append(Rope::from_str("\n".to_string().repeat(dividing_newlines_count).as_str()));
        }
        self.new_data = true;
        self.rendered_text.remove(rendered_text_message_start_index..);
        self.rendered_text.append(message.rendered.stylized.clone());
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
  buffer: Vec<u8>,
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
      StyleComponent::Grid,
      //StyleComponent::LineNumbers,
      //StyleComponent::Changes,
      //StyleComponent::Rule,
      //StyleComponent::Default,
      //StyleComponent::Snip,
      //StyleComponent::Plain,
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
    let buffer: Vec<u8> = Vec::new();
    BatRenderer { config, assets, buffer }
  }

  fn render_error(err: &bat::error::Error, _write: &mut dyn std::io::Write) {
    trace_dbg!("bat rendering error: {}", err);
  }

  fn render_message_bat(&mut self, start_line: usize, line_count: usize, text: &Rope) -> String {
    //    fn render_message_bat(&self, content: &str) -> String {
    self.config.visible_lines =
      VisibleLines::Ranges(LineRanges::from(vec![LineRange::new(start_line, start_line + line_count)]));

    let controller = Controller::new(&self.config, &self.assets);
    let mut buffer = String::new();
    //let input = Input::from_bytes(content.as_bytes());
    //trace_dbg!("render_message_bat: {:?}", self.rendered_bytes);

    text.bytes().skip(self.buffer.len()).for_each(|b| self.buffer.push(b));
    let input = Input::from_bytes(self.buffer.as_slice());
    //trace_dbg!("render_message_bat: {:?}", self.rendered_bytes);
    controller.run_with_error_handler(vec![input.into()], Some(&mut buffer), Self::render_error).unwrap();
    //trace_dbg!("render_message_bat: {:?}", buffer);
    buffer
  }
}

fn search_and_replace(rope: &mut Rope, search_pattern: &str, replacement_text: &str) {
  const BATCH_SIZE: usize = 256;
  let replacement_text_len = replacement_text.chars().count();

  let mut head = 0; // Keep track of where we are between searches
  let mut matches = Vec::with_capacity(BATCH_SIZE);
  loop {
    // Collect the next batch of matches.  Note that we don't use
    // `Iterator::collect()` to collect the batch because we want to
    // re-use the same Vec to avoid unnecessary allocations.
    matches.clear();
    for m in SearchIter::from_rope_slice(&rope.slice(head..), search_pattern).take(BATCH_SIZE) {
      matches.push(m);
    }

    // If there are no matches, we're done!
    if matches.is_empty() {
      break;
    }

    // Replace the collected matches.
    let mut index_diff: isize = 0;
    for &(start, end) in matches.iter() {
      // Get the properly offset indices.
      let start_d = (head as isize + start as isize + index_diff) as usize;
      let end_d = (head as isize + end as isize + index_diff) as usize;

      // Do the replacement.
      rope.remove(start_d..end_d);
      rope.insert(start_d, replacement_text);

      // Update the index offset.
      let match_len = (end - start) as isize;
      index_diff = index_diff - match_len + replacement_text_len as isize;
    }

    // Update head for next iteration.
    head = (head as isize + index_diff + matches.last().unwrap().1 as isize) as usize;
  }
}

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
  SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

static RESOURCE_HANDLER: OnceLock<DispatchingResourceHandler> = OnceLock::new();

fn resource_handler() -> &'static DispatchingResourceHandler {
  RESOURCE_HANDLER.get_or_init(|| {
    let handlers: Vec<Box<dyn ResourceUrlHandler>> = vec![];
    //let handlers: Vec<Box<dyn ResourceUrlHandler>> = vec![Box::new(FileResourceHandler::new(TEST_READ_LIMIT))];
    DispatchingResourceHandler::new(handlers)
  })
}

pub fn render_markdown_to_string(input: String) -> String {
  // let theme = Theme::default();
  // theme.html_block_style = Style::new().fg_color(Some(AnsiColor::Green.into()));
  //
  // Theme {
  //   html_block_style: Style::new().fg_color(Some(AnsiColor::Green.into())),
  //   inline_html_style: Style::new().fg_color(Some(AnsiColor::Green.into())),
  //   code_style: Style::new().fg_color(Some(AnsiColor::Yellow.into())),
  //   link_style: Style::new().fg_color(Some(AnsiColor::Blue.into())),
  //   image_link_style: Style::new().fg_color(Some(AnsiColor::Magenta.into())),
  //   rule_color: AnsiColor::Green.into(),
  //   code_block_border_color: AnsiColor::Green.into(),
  //   heading_style: Style::new().fg_color(Some(AnsiColor::Blue.into())).bold(),
  // };
  let settings = Settings {
    terminal_capabilities: TerminalProgram::ITerm2.capabilities(),
    terminal_size: TerminalSize::default(),
    theme: Theme::default(),
    syntax_set: syntax_set(),
  };

  let parser = Parser::new_ext(&input, Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH);
  let abs_path = std::fs::canonicalize(Path::new("./")).unwrap();
  let base_dir = abs_path.parent().expect("Absolute file name must have a parent!");
  let mut sink = Vec::new();
  let env = Environment { hostname: "HOSTNAME".to_string(), ..Environment::for_local_directory(&base_dir).unwrap() };
  pulldown_cmark_mdcat::push_tty(&settings, &env, resource_handler(), &mut sink, parser).unwrap();
  String::from_utf8(sink).unwrap()
}

struct SearchIter<'a> {
  char_iter: Chars<'a>,
  search_pattern: &'a str,
  search_pattern_char_len: usize,
  cur_index: usize,                           // The current char index of the search head.
  possible_matches: Vec<std::str::Chars<'a>>, // Tracks where we are in the search pattern for the current possible matches.
}

impl<'a> SearchIter<'a> {
  fn from_rope_slice<'b>(slice: &'b RopeSlice, search_pattern: &'b str) -> SearchIter<'b> {
    assert!(!search_pattern.is_empty(), "Can't search using an empty search pattern.");
    SearchIter {
      char_iter: slice.chars(),
      search_pattern,
      search_pattern_char_len: search_pattern.chars().count(),
      cur_index: 0,
      possible_matches: Vec::new(),
    }
  }
}

impl<'a> Iterator for SearchIter<'a> {
  type Item = (usize, usize);

  // Return the start/end char indices of the next match.
  fn next(&mut self) -> Option<(usize, usize)> {
    #[allow(clippy::while_let_on_iterator)]
    while let Some(next_char) = self.char_iter.next() {
      self.cur_index += 1;

      // Push new potential match, for a possible match starting at the
      // current char.
      self.possible_matches.push(self.search_pattern.chars());

      // Check the rope's char against the next character in each of
      // the potential matches, removing the potential matches that
      // don't match.  We're using indexing instead of iteration here
      // so that we can remove the possible matches as we go.
      let mut i = 0;
      while i < self.possible_matches.len() {
        let pattern_char = self.possible_matches[i].next().unwrap();
        if next_char == pattern_char {
          if self.possible_matches[i].clone().next().is_none() {
            // We have a match!  Reset possible matches and
            // return the successful match's char indices.
            let char_match_range = (self.cur_index - self.search_pattern_char_len, self.cur_index);
            self.possible_matches.clear();
            return Some(char_match_range);
          } else {
            // Match isn't complete yet, move on to the next.
            i += 1;
          }
        } else {
          // Doesn't match, remove it.
          let _ = self.possible_matches.swap_remove(i);
        }
      }
    }

    None
  }
}
