use bat::{
  assets::HighlightingAssets,
  config::Config,
  controller::Controller,
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
use ropey::Rope;

#[derive(Default, Debug)]
pub struct SessionView {
  pub renderer: BatRenderer<'static>,
  pub window_width: usize,
  pub render_conditions: (usize, usize, usize, usize),
  pub rendered_view: String,
  pub rendered_text: Rope,
}

impl SessionView {
  pub fn set_window_width(&mut self, width: usize, messages: &mut [MessageContainer]) {
    let new_value = width - 6;
    if self.window_width != new_value {
      trace_dbg!("setting window width to {}", new_value);

      self.window_width = new_value;
      self.renderer = BatRenderer::new(self.window_width);
      messages.iter_mut().for_each(|m| {
        m.finished = false;
      });
    }
  }
  pub fn get_stylized_rendered_slice(&mut self, start_line: usize, line_count: usize, vertical_scroll: usize) -> &str {
    if (start_line, line_count, vertical_scroll, self.rendered_text.len_lines()) != self.render_conditions {
      self.render_conditions = (start_line, line_count, vertical_scroll, self.rendered_text.len_lines());
      let text =
        self.rendered_text.lines().skip(start_line).take(line_count).map(|c| c.to_string()).collect::<String>();
      let wrapped_text = bwrap::wrap!(&text, self.window_width);
      self.rendered_view =
        "\n".repeat(vertical_scroll).to_string() + &self.renderer.render_message_bat(wrapped_text.as_str());
    }
    &self.rendered_view
  }

  pub fn post_process_new_messages(&mut self, session_data: &mut SessionData) {
    let dividing_newlines_count = 2;
    session_data.messages.iter_mut().for_each(|message| {
      let rendered_text_message_start_index = self.rendered_text.len_chars() - message.rendered.stylized.len_chars();
      if !message.finished {
        message.rendered = RenderedChatMessage::from(&message.message);
        message.rendered.stylized =
          Rope::from_str(bwrap::wrap!(&message.rendered.content, self.window_width - 3).as_str());
        if message.rendered.finish_reason.is_some() {
          message.finished = true;
          message.rendered.stylized.append(Rope::from_str("\n".to_string().repeat(dividing_newlines_count).as_str()));
        }
        self.rendered_text.remove(rendered_text_message_start_index..);
        self.rendered_text.append(message.rendered.stylized.clone());
      }
    });
  }
}

pub struct BatRenderer<'a> {
  assets: HighlightingAssets,
  config: Config<'a>,
  //controller: Controller<'a>,
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
      StyleComponent::LineNumbers,
      //StyleComponent::Changes,
      StyleComponent::Rule,
      //StyleComponent::Default,
      //StyleComponent::Snip,
      //StyleComponent::Plain,
    ]);
    let config: Config<'static> = Config {
      colored_output: true,
      language: Some("markdown"),
      style_components,
      show_nonprintable: false,
      tab_width: 0,
      wrapping_mode: bat::WrappingMode::NoWrapping(false),
      use_italic_text: true,
      term_width,
      paging_mode: bat::PagingMode::Never,
      true_color: true,
      ..Default::default()
    };
    let assets = HighlightingAssets::from_binary();
    BatRenderer { config, assets }
  }

  fn render_message_bat(&self, content: &str) -> String {
    let controller = Controller::new(&self.config, &self.assets);
    let mut buffer = String::new();
    let input = Input::from_bytes(content.as_bytes());
    controller.run(vec![input.into()], Some(&mut buffer)).unwrap();
    buffer
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
