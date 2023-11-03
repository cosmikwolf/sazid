use pulldown_cmark::{Options, Parser};
use pulldown_cmark_mdcat::resources::*;
use pulldown_cmark_mdcat::terminal::{TerminalProgram, TerminalSize};
use pulldown_cmark_mdcat::Settings;
use pulldown_cmark_mdcat::{Environment, Theme};
use std::path::Path;
use std::sync::OnceLock;
use syntect::parsing::SyntaxSet;

// static TEST_READ_LIMIT: u64 = 5_242_880;

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
