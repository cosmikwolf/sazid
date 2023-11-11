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

use super::{
  messages::{ChatMessage, MessageContainer, RenderedChatMessage},
  session_data::SessionData,
};

// static TEST_READ_LIMIT: u64 = 5_242_880;
#[derive(Default, Debug)]
pub struct SessionView {
  pub renderer: BatRenderer<'static>,
  pub window_width: usize,
}

impl SessionView {
  pub fn set_window_width(&mut self, width: usize, messages: &mut Vec<MessageContainer>) {
    if self.window_width != width {
      self.window_width = width;
      messages.iter_mut().for_each(|m| m.wrap_stylized_text(width));
    }
  }
  pub fn post_process_new_messages(&self, session_data: &mut SessionData) {
    session_data.rendered_text = session_data
      .messages
      .iter_mut()
      .flat_map(|message| {
        if !message.finished {
          // trace_dbg!("post_process_new_messages: processing message {:#?}", message.message);
          message.rendered = RenderedChatMessage::from(&ChatMessage::from(message.clone()));
          // message.render_message_pulldown_cmark(true);
          message.rendered.stylized =
            Some(self.renderer.render_message_bat(message.rendered.content.clone().unwrap_or_default().as_str()));
          message.wrap_stylized_text(self.window_width);
          if message.rendered.finish_reason.is_some() {
            message.finished = true;
            // trace_dbg!("post_process_new_messages: finished message {:#?}", message);
          }
        }
        message.rendered.wrapped_lines.iter().map(|wl| wl.as_str()).collect::<Vec<&str>>()
      })
      .collect::<Vec<&str>>()
      .join("\n");
  }
}
pub fn get_display_text(index: usize, count: usize, messages: Vec<MessageContainer>) -> Vec<String> {
  messages.iter().map(|m| m.rendered.wrapped_lines.clone()).skip(index).take(count).flatten().collect::<Vec<String>>()
}
pub struct BatRenderer<'a> {
  assets: HighlightingAssets,
  config: Config<'a>,
}

impl<'a> Default for BatRenderer<'a> {
  fn default() -> Self {
    BatRenderer::new()
  }
}

impl<'a> std::fmt::Debug for BatRenderer<'a> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BatRenderer").field("assets", &self.assets).field("config", &self.config).finish()
  }
}
// static BAT_RENDERER: Lazy<BatRenderer<'static>> = Lazy::new(|| BatRenderer::new());
// static HIGHIGHT_ASSETS: OnceCell<HighlightingAssets> = OnceCell::new(|| HighlightingAssets::from_binary());
// static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
//
// fn syntax_set() -> &'static SyntaxSet {
//   SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
// }
//
// static BAT_RENDERER: OnceLock<BatRenderer> = OnceLock::new();
//
// fn bat_renderer() -> &'static BatRenderer<'static> {
//   BAT_RENDERER.get_or_init(BatRenderer::new())
// }
//
// static HIGHIGHT_ASSETS: <HighlightingAssets> = OnceLock::new();
//
// fn highlighting_assets() -> &'static HighlightingAssets {
//   HIGHIGHT_ASSETS.get_or_init(HighlightingAssets::new(
//     SerializedSyntaxSet::FromBinary(get_serialized_integrated_syntaxset()),
//     get_integrated_themeset(),
//   ))
// }

impl<'a> BatRenderer<'a> {
  fn new() -> Self {
    let style_components = StyleComponents::new(&[
      StyleComponent::Header,
      StyleComponent::Grid,
      StyleComponent::LineNumbers,
      StyleComponent::Changes,
      StyleComponent::Rule,
      StyleComponent::Snip,
      StyleComponent::Plain,
    ]);
    let config = Config { colored_output: true, language: Some("markdown"), style_components, ..Default::default() };
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
