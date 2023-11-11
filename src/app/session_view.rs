use bat::{
  assets::HighlightingAssets,
  config::Config,
  controller::Controller,
  style::{StyleComponent, StyleComponents},
  Input,
};

use anstyle::{AnsiColor, Color, Style};
use pulldown_cmark::{Options, Parser};
use pulldown_cmark_mdcat::resources::*;
use pulldown_cmark_mdcat::terminal::{TerminalProgram, TerminalSize};
use pulldown_cmark_mdcat::Settings;
use pulldown_cmark_mdcat::{Environment, Theme};
use std::path::Path;
use std::sync::OnceLock;
use syntect::parsing::SyntaxSet;

// static TEST_READ_LIMIT: u64 = 5_242_880;
#[derive(Default, Debug)]
pub struct SessionView {
  pub renderer: BatRenderer<'static>,
}

pub struct BatRenderer<'a> {
  style_components: StyleComponents,
  assets: HighlightingAssets,
  config: Config<'a>,
  controller: Controller<'a>,
}

impl<'a> Default for BatRenderer<'a> {
  fn default() -> Self {
    BatRenderer::new()
  }
}

impl<'a> std::fmt::Debug for BatRenderer<'a> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BatRenderer")
      .field("style_components", &self.style_components)
      .field("assets", &self.assets)
      .field("config", &self.config)
      .finish()
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
    let controller = Controller::new(&config, &assets);
    BatRenderer { style_components, config, assets, controller }
  }

  fn render_message_bat(&mut self, content: &str) -> String {
    let mut buffer = String::new();
    let input = Input::from_bytes(content.as_bytes());
    self.controller.run(vec![input.into()], Some(&mut buffer)).unwrap();
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
