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
use bwrap::{EasyWrapper, WrapStyle};
use ropey::Rope;
// static TEST_READ_LIMIT: u64 = 5_242_880;
#[derive(Default, Debug)]
pub struct SessionView {
  pub renderer: BatRenderer<'static>,
  pub window_width: usize,
  pub rendered_text: Rope,
}

impl SessionView {
  pub fn set_window_width(&mut self, width: usize, messages: &mut [MessageContainer]) {
    if self.window_width != width {
      trace_dbg!("setting window width to {}", width);

      self.window_width = width - 6;
      self.renderer = BatRenderer::new(self.window_width);
      messages.iter_mut().for_each(|m| {
        m.finished = false;
        //m.wrap_stylized_text(width)
      });
    }
  }
  // pub fn wrap_stylized_text(&self, message: &mut MessageContainer) {
  //   let mut wrapper = EasyWrapper::new(&message.rendered.stylized, self.window_width).expect("bwrap init");
  //   let w = wrapper.wrap_use_style(WrapStyle::MayBrk(None, None)).expect("bwrap wrap");
  //   //self.rendered.wrapped_lines = stylized_text.split('\n').map(|s| s.to_string()).collect()
  // }
  pub fn post_process_new_messages(&mut self, session_data: &mut SessionData) {
    session_data.messages.iter_mut().for_each(|message| {
      let rendered_text_message_start_index = self.rendered_text.len_chars() - message.rendered.stylized.len_chars();
      trace_dbg!("post_process_new_messages: processing message {:#?}", message.message);
      trace_dbg!("original_stylized_char_count  {:#?}", rendered_text_message_start_index);
      if !message.finished {
        trace_dbg!("post_process_new_messages: processing message {:#?}", message.rendered.stylized);
        message.rendered = RenderedChatMessage::from(&message.message);
        //trace_dbg!("{:#?}", message.rendered);
        message.rendered.stylized = Rope::from_str(
          //self.renderer.render_message_bat(bwrap::wrap_maybrk!(&message.rendered.content, self.window_width).as_str()),
          self.renderer.render_message_bat(&message.rendered.content).as_str(),
        );
        //  self.renderer.render_message_bat(&message.rendered.content);
        //self.wrap_stylized_text(message);
        if message.rendered.finish_reason.is_some() {
          message.finished = true;
          message.rendered.stylized.append(Rope::from_str("\n\n"));
          trace_dbg!("post_process_new_messages: finished message {:#?}", message.rendered.stylized);
        }
        trace_dbg!(
          "appending stylized chars: {:#?}   rendered_length: {:#?}",
          message.rendered.stylized.len_chars(),
          self.rendered_text.len_chars()
        );
        // Insert the replacement text
        self.rendered_text.remove(rendered_text_message_start_index..);
        self.rendered_text.append(message.rendered.stylized.clone());
        trace_dbg!("appended stylized to rendered_text: {:#?}", self.rendered_text.len_chars());
      }

      // message.rendered.stylized.chars().for_each(|char| {
      //   char_count += 1;
      //   if char_count > self.rendered_text.chars().count() {
      //     self.rendered_text.push(char);
      //   }
      // })
      //message.rendered.wrapped_lines.iter().map(|wl| wl.as_str()).collect::<Vec<&str>>()
    });
  }
}

// pub fn update_rendered_chat_message(
//   original: RenderedChatMessage,
//   incoming: RenderedChatMessage,
// ) -> RenderedChatMessage {
//   let new_content = &incoming.content.unwrap_or_default()[original.content.unwrap_or_default().len()..];
//
//   let mut wrapper = EasyWrapper::new(&message.rendered.stylized, self.window_width).expect("bwrap init");
//   let wrapped_content = wrapper.wrap_use_style(WrapStyle::MayBrk(None, None)).expect("bwrap wrap");
//
//   RenderedChatMessage {
//     role: incoming.role,
//     content: incoming.content,
//     wrapped_content,
//     stylized: (),
//     function_call: (),
//     name: incoming.name,
//     finish_reason: (),
//   }
// }

// pub fn get_display_text(index: usize, count: usize, messages: Vec<MessageContainer>) -> Vec<String> {
//   let mut lines = vec!["".to_string(); index];
//   messages.iter().map(|m| m.rendered.stylized.lines().skip(index).take(count).map(|l| lines.push(l.to_string())));
//   let full_line_count = messages.iter().map(|m| m.rendered.stylized.matches('\n').count()).sum::<usize>();
//   lines.append(&mut vec!["".to_string(); full_line_count - index - count]);
//   lines
// }
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
      tab_width: 4,
      wrapping_mode: bat::WrappingMode::NoWrapping(true),
      use_italic_text: true,
      term_width,
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
