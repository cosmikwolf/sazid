use std::{iter, sync::Arc};

use super::Context;
use crate::{
  compositor::{self, Compositor},
  job::Callback,
  ui::{self, overlay::overlaid},
  widgets::{
    plaintext_reflow::{LineComposerStr, WordWrapperStr},
    reflow::{LineComposer, WordWrapper},
  },
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
  chat_completion_request_message_content_as_str,
  chat_completion_request_message_tool_calls_as_str,
};
use tui::text::{Span, Spans, Text};

use helix_core::{syntax, Rope};
use unicode_segmentation::UnicodeSegmentation;

/// Gets the first language server that is attached to a document which supports a specific feature.
/// If there is no configured language server that supports the feature, this displays a status message.
/// Using this macro in a context where the editor automatically queries the LSP
/// (instead of when the user explicitly does so via a keybind like `gd`)
/// will spam the "No configured language server supports \<feature>" status message confusingly.

#[derive(Clone)]
pub enum ChatMessageType {
  Error(String),
  Chat(ChatCompletionRequestMessage),
}

#[derive(Clone)]
pub struct ChatMessageItem {
  pub id: Option<i64>,
  pub formatted_line_char_len: Vec<usize>,
  pub plain_text: Rope,
  pub select_range: Option<Range>,
  pub config_loader: Arc<ArcSwap<syntax::Loader>>,
  pub chat_message: ChatMessageType,
  pub line_widths: Vec<u16>,
  pub plaintext_wrapped_width: u16,
  pub formatted_line_widths: Vec<(usize, String)>,
  pub plaintext_line_widths: Vec<(usize, String)>,
  pub rendered_area: Option<Rect>,
}

impl ChatMessageItem {
  pub fn new_chat(
    id: i64,
    message: ChatCompletionRequestMessage,
    config_loader: Arc<ArcSwap<syntax::Loader>>,
  ) -> Self {
    let id = Some(id);
    let message = ChatMessageType::Chat(message);
    let select_range = None;
    let formatted_line_char_len = Vec::new();
    Self {
      id,
      formatted_line_char_len,
      config_loader,
      chat_message: message.clone(),
      select_range,
      plain_text: Rope::new(),
      line_widths: Vec::new(),
      plaintext_wrapped_width: 0,
      formatted_line_widths: vec![],
      plaintext_line_widths: vec![],
      rendered_area: None,
    }
  }

  pub fn new_error(
    message: String,
    config_loader: Arc<ArcSwap<syntax::Loader>>,
  ) -> Self {
    let id = None;
    let message = ChatMessageType::Error(message);
    let select_range = None;
    let formatted_line_char_len = Vec::new();
    Self {
      id,
      formatted_line_char_len,
      config_loader,
      chat_message: message.clone(),
      select_range,
      plain_text: Rope::new(),
      line_widths: Vec::new(),
      plaintext_wrapped_width: 0,
      formatted_line_widths: vec![],
      plaintext_line_widths: vec![],
      rendered_area: None,
    }
  }

  pub fn update_message(&mut self, message: ChatMessageType) {
    self.chat_message = message;
  }
  pub fn cache_wrapped_yank_text(&mut self, width: u16) {
    let text = self.format_to_text(None);

    let line_widths: Vec<u16> =
      text.lines.iter().map(|spans| spans.width() as u16).collect();
    //
    // let text = text.lines.iter().flat_map(|spans| {
    //   spans
    //     .0
    //     .iter()
    //     .flat_map(|span| {
    //       // log::info!("span: {:#?}", span);
    //       span.content.as_ref().split_word_bounds()
    //     })
    //     .chain(iter::once("\n"))
    // });
    //
    // let trim = false;
    // let mut line_composer: Box<dyn LineComposerStr> =
    //   Box::new(WordWrapperStr::new(Box::new(text), width, trim));
    //
    // // log::error!("width: {}", width);
    // let mut plain_text = Rope::new();
    // use helix_core::unicode::width::UnicodeWidthStr;
    // while let Some((mut symbol, length)) = line_composer.next_line() {
    //   // log::info!(
    //   //   "symbol: {:#?}  width: {}  length:{}",
    //   //   symbol,
    //   //   symbol.width(),
    //   //   length
    //   // );
    //   if symbol.is_empty() {
    //     symbol = " ";
    //   }
    //   plain_text.insert(plain_text.len_chars(), symbol);
    //   plain_text.insert(plain_text.len_chars(), "\n");
    // }
    // plain_text.remove(plain_text.len_chars() - 1..);
    // plain_text
    //   .insert(plain_text.len_chars(), &"\n".repeat(row_spacing as usize));
    // drop(line_composer);

    // log::warn!("text: {}", format!("{}", plain_text));
    self.plain_text = Rope::from(
      text
        .lines
        .iter()
        .map(String::from)
        .chain(iter::once("\n".to_string()))
        .collect::<String>(),
    );
    self.plaintext_wrapped_width = width;
    self.line_widths = line_widths;
  }

  fn format_to_text(&self, theme: Option<&Theme>) -> tui::text::Text {
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
        Style::default()
          .fg(Color::Red)
          .add_modifier(helix_view::theme::Modifier::BOLD),
        "ERROR".to_string(),
      ),
    };
    // log::warn!("content: {}\nheader: {}", self.content(), header);
    let header = Spans::from(vec![Span::styled(header, style)]);
    let mut lines = vec![header];
    let text = MarkdownRenderer::parse(
      self.content(),
      theme,
      self.config_loader.clone(),
    );
    lines.extend(text);

    if let Some(tool_calls) = self.tool_calls() {
      tool_calls.iter().for_each(|(tool_name, tool_args)| {
        lines.extend(Text::from(Spans::from(vec![
          Span::styled("   Tool Call: ", Style::default().fg(Color::White)),
          Span::styled(*tool_name, Style::default().fg(Color::Cyan)),
        ])));
        lines.extend(Text::from(Spans::from(vec![
          Span::styled("   Arguments: ", Style::default().fg(Color::White)),
          Span::styled(*tool_args, Style::default().fg(Color::Cyan)),
        ])));
      })
    }
    lines.into()
  }

  pub fn content(&self) -> &str {
    match &self.chat_message {
      ChatMessageType::Chat(message) => {
        chat_completion_request_message_content_as_str(message)
      },
      ChatMessageType::Error(error) => error,
    }
  }
  pub fn tool_calls(&self) -> Option<Vec<(&str, &str)>> {
    match &self.chat_message {
      ChatMessageType::Chat(message) => {
        chat_completion_request_message_tool_calls_as_str(message)
      },
      ChatMessageType::Error(_) => None,
    }
  }
}

impl ui::markdownmenu::MarkdownItem for ChatMessageItem {
  /// Current working directory.
  type Data = String;

  fn format(
    &self,
    _data: &Self::Data,
    theme: Option<&Theme>,
  ) -> tui::text::Text {
    self.format_to_text(theme)
  }
}

pub fn session_messages(cx: &mut Context) {
  let (_view, _doc) = current!(cx.editor);

  let messages_fut = futures_util::future::ready(
    cx.session
      .messages
      .clone()
      .iter()
      .map(|message| {
        ChatMessageItem::new_chat(
          message.message_id,
          message.message.clone(),
          cx.editor.syn_loader.clone(),
        )
      })
      .collect::<Vec<_>>(),
  );

  let session_callback =
    |_context: &mut compositor::Context,
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
        session_callback,
      );
      compositor.replace_or_push("markdown text", overlaid(markdown_session));
      // compositor.push(Box::new(textbox))
      // compositor.replace_or_push("textbox test", Popup::new("textbox", textbox))
    };

    Ok(Callback::EditorCompositor(Box::new(call)))
  });
}
