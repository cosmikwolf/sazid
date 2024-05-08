use std::sync::Arc;

use super::Context;
use crate::{
  compositor::{self, Compositor},
  job::Callback,
  ui::{self, overlay::overlaid},
  widgets::{paragraph::Wrap, table::MessageCell},
};

use crate::ui::MarkdownRenderer;
use arc_swap::ArcSwap;
use async_openai::types::ChatCompletionRequestMessage;
use helix_lsp::lsp::Range;
use helix_view::{
  graphics::Rect,
  theme::{Color, Style},
  Editor, Theme,
};
use sazid::app::messages::{
  chat_completion_request_message_content_as_str, chat_completion_request_message_tool_calls_as_str,
};
use tui::{
  buffer::Buffer,
  text::{Span, Spans, Text},
};

use helix_core::{syntax, Rope};

#[derive(Debug, Clone, PartialEq)]
pub enum ChatMessageType {
  Error(String),
  Chat(ChatCompletionRequestMessage),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatMessageItem {
  pub id: Option<i64>,
  pub formatted_line_char_len: Vec<usize>,
  pub plain_text: Rope,
  pub raw_text: String,
  pub tool_call_text: Box<Vec<Spans<'static>>>,
  pub formatted_text: Box<Text<'static>>,
  pub select_range: Option<Range>,
  pub chat_message: ChatMessageType,
  pub line_widths: Vec<u16>,
  pub plaintext_wrapped_width: u16,
  pub formatted_line_widths: Vec<(usize, String)>,
  pub plaintext_line_widths: Vec<(usize, String)>,
  pub rendered_area: Option<Rect>,
  pub start_idx: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChatStringItem {
  pub text: String,
  pub formatted_text: Box<Text<'static>>,
}

impl ChatStringItem {
  pub fn new(text: String, style: Style) -> Self {
    let formatted_text =
      Box::new(Text::from(vec![Spans::from(vec![Span::styled(text.clone(), style)])]));
    Self { text, formatted_text }
  }
}
impl ChatMessageItem {
  pub fn new_chat(id: i64, message: ChatCompletionRequestMessage) -> Self {
    let id = Some(id);
    let message = ChatMessageType::Chat(message);
    let select_range = None;
    let formatted_line_char_len = Vec::new();
    Self {
      id,
      formatted_line_char_len,
      chat_message: message.clone(),
      select_range,
      plain_text: Rope::new(),
      raw_text: String::new(),
      tool_call_text: Box::new(Vec::new()),
      formatted_text: Box::new(Text::from(Vec::new())),
      line_widths: Vec::new(),
      plaintext_wrapped_width: 0,
      formatted_line_widths: vec![],
      plaintext_line_widths: vec![],
      rendered_area: None,
      start_idx: 0,
    }
  }

  pub fn new_error(message: String) -> Self {
    let id = None;
    let message = ChatMessageType::Error(message);
    let select_range = None;
    let formatted_line_char_len = Vec::new();
    Self {
      id,
      formatted_line_char_len,
      chat_message: message.clone(),
      select_range,
      plain_text: Rope::new(),
      formatted_text: Box::new(Text::from(Vec::new())),
      tool_call_text: Box::new(Vec::new()),
      raw_text: String::new(),
      line_widths: Vec::new(),
      plaintext_wrapped_width: 0,
      formatted_line_widths: vec![],
      plaintext_line_widths: vec![],
      rendered_area: None,
      start_idx: 0,
    }
  }

  pub fn get_wrapped_height(&self, width: u16) -> usize {
    if self.plaintext_wrapped_width == width {
      self.plain_text.len_lines()
    } else {
      log::error!(
        "need to update wrapping before trying to get wrapped height, or else it is not up to date"
      );
      self.plain_text.len_lines()
    }
  }

  pub fn update_message(&mut self, message: ChatMessageType) {
    self.chat_message = message;
  }

  pub fn update_wrapped_plain_text_if_necessary(
    &mut self,
    width: u16,
    config_loader: &Arc<ArcSwap<syntax::Loader>>,
  ) {
    if self.plaintext_wrapped_width != width {
      self.cache_wrapped_plain_text(width, config_loader)
    }
  }

  pub fn cache_wrapped_plain_text(
    &mut self,
    width: u16,
    config_loader: &Arc<ArcSwap<syntax::Loader>>,
  ) {
    let style = Style::default();
    let area = Rect::new(0, 0, width, 0);
    let buf = &mut Buffer::empty(area);
    self.plain_text = if let Some(plain_text) = MessageCell::format_text(
      buf,
      self.formatted_text.clone(),
      true,
      false,
      style,
      Some(Wrap { trim: false }),
      area,
      tui::layout::Alignment::Left,
      None,
      0,
      None,
      None,
    ) {
      plain_text
    } else {
      self.plain_text.clone()
    };

    // log::warn!("plain_text: {}", self.plain_text);
    self.plaintext_wrapped_width = width;
    // self.line_widths =
    //   self.plain_text.lines().map(|l| l.len_chars() as u16).collect();
  }

  pub fn format_chat_message(
    &'static mut self,
    theme: Option<&Theme>,
    config_loader: Arc<ArcSwap<syntax::Loader>>,
  ) {
    let (style, header) = match self.chat_message {
      ChatMessageType::Chat(ChatCompletionRequestMessage::System(_)) => {
        (
          Style::default()
        .fg(Color::Magenta)
        // .add_modifier(helix_view::theme::Modifier::ITALIC)
        .add_modifier(helix_view::theme::Modifier::BOLD),
          "System".to_string(),
        )
      },
      ChatMessageType::Chat(ChatCompletionRequestMessage::User(_)) => {
        (
          Style::default()
        .fg(Color::Green)
        // .add_modifier(helix_view::theme::Modifier::ITALIC)
        .add_modifier(helix_view::theme::Modifier::BOLD),
          "User".to_string(),
        )
      },
      ChatMessageType::Chat(ChatCompletionRequestMessage::Assistant(_)) => {
        (
          Style::default()
        .fg(Color::Blue)
        // .add_modifier(helix_view::theme::Modifier::ITALIC)
        .add_modifier(helix_view::theme::Modifier::BOLD),
          "Assistant".to_string(),
        )
      },
      ChatMessageType::Chat(ChatCompletionRequestMessage::Tool(_)) => {
        (
          Style::default()
        .fg(Color::Yellow)
        // .add_modifier(helix_view::theme::Modifier::ITALIC)
        .add_modifier(helix_view::theme::Modifier::BOLD),
          "Tool".to_string(),
        )
      },
      ChatMessageType::Chat(ChatCompletionRequestMessage::Function(_)) => {
        (
          Style::default()
        .fg(Color::LightYellow)
        // .add_modifier(helix_view::theme::Modifier::ITALIC)
        .add_modifier(helix_view::theme::Modifier::BOLD),
          "Function".to_string(),
        )
      },
      ChatMessageType::Error(_) => (
        Style::default().fg(Color::Red).add_modifier(helix_view::theme::Modifier::BOLD),
        "ERROR".to_string(),
      ),
    };

    // log::warn!("content: {}\nheader: {}", self.content(), header);
    let header = Spans::from(vec![Span::styled(header, style)]);

    if self.formatted_text.lines.len() == 0 {
      self.formatted_text.lines.push(header)
    }

    self.raw_text =
      if let ChatMessageType::Chat(ChatCompletionRequestMessage::Tool(_)) = &self.chat_message {
        if self.content().lines().count() > 1 {
          "tool call response content".to_string()
        } else {
          self.content()
        }
      } else {
        self.content()
      };

    let skip_events = Some(self.formatted_text.lines.len() - 1); // header length is always 1

    let new_lines =
      MarkdownRenderer::parse(&self.raw_text, theme, config_loader.clone(), skip_events);
    self.formatted_text.extend(new_lines);

    let tool_calls = match &self.chat_message {
      ChatMessageType::Chat(message) => chat_completion_request_message_tool_calls_as_str(message),
      ChatMessageType::Error(_) => None,
    };

    self.formatted_text.extend(Self::get_tool_call_text(tool_calls))
  }

  pub fn content(&self) -> String {
    match &self.chat_message {
      ChatMessageType::Chat(message) => {
        chat_completion_request_message_content_as_str(message).to_string()
      },
      ChatMessageType::Error(error) => error.to_string(),
    }
  }
  pub fn get_tool_call_text<'a>(tool_calls: Option<Vec<(&'a str, &'a str)>>) -> Vec<Spans<'a>> {
    match tool_calls {
      Some(tool_calls) => tool_calls
        .iter()
        .flat_map(|(tool_name, tool_args)| {
          vec![
            Spans::from(vec![
              Span::styled("   Tool Call: ", Style::default().fg(Color::White)),
              Span::styled(*tool_name, Style::default().fg(Color::Cyan)),
            ]),
            Spans::from(vec![
              Span::styled("   Arguments: ", Style::default().fg(Color::White)),
              Span::styled(*tool_args, Style::default().fg(Color::Cyan)),
            ]),
          ]
        })
        .collect::<Vec<Spans<'_>>>(),
      None => vec![],
    }
  }
}

impl ui::markdownmenu::MarkdownItem for ChatMessageItem {
  /// Current working directory.
  type Data = String;

  fn format(&self, _data: &Self::Data, _theme: Option<&Theme>) -> tui::text::Text {
    // self.format_to_text(theme)
    Text::from("")
  }
}

pub fn session_messages(cx: &mut Context) {
  let (_view, _doc) = current!(cx.editor);

  let messages_fut = futures_util::future::ready(
    cx.session
      .messages
      .clone()
      .iter()
      .map(|message| ChatMessageItem::new_chat(message.message_id, message.message.clone()))
      .collect::<Vec<_>>(),
  );

  let session_callback = |_context: &mut compositor::Context,
                          _message: &ChatMessageItem,
                          _action: helix_view::editor::Action| {};

  cx.jobs.callback(async move {
    // let mut messages = Vec::new();
    // // TODO if one symbol request errors, all other requests are discarded (even if they're valid)
    // while let Some(mut msgs) = messages_fut.try_next().await? {
    //   messages.append(&mut msgs);
    // }
    let messages = messages_fut.await;
    let call = move |editor: &mut Editor, compositor: &mut Compositor| {
      // let editor_data = get_chat_message_text(&messages[0].message);
      // let markdown_message = MarkdownMenu::new(
      //   messages.clone(),
      //   editor_data.clone(),
      //   callback_fn,
      //   editor.syn_loader.clone(),
      //   Some(editor.theme.clone()),
      // );
      //
      log::debug!("messages count: {}", messages.len());

      let editor_data = String::new();
      let markdown_session = ui::SessionView::new(
        messages,
        Some(editor.theme.clone()),
        editor_data,
        editor.syn_loader.clone(),
        session_callback,
      );
      compositor.replace_or_push("markdown text", overlaid(markdown_session));
      // compositor.push(Box::new(textbox))
      // compositor.replace_or_push("textbox test", Popup::new("textbox", textbox))
    };

    Ok(Callback::EditorCompositor(Box::new(call)))
  });
}
