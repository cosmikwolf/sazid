use std::sync::Arc;

use super::Context;
use crate::{
  compositor::{self, Compositor},
  job::Callback,
  ui::{self, overlay::overlaid},
};

use crate::ui::MarkdownRenderer;
use arc_swap::ArcSwap;
use async_openai::types::ChatCompletionRequestMessage;
use helix_view::{
  theme::{Color, Style},
  Editor, Theme,
};
use sazid::app::messages::{
  chat_completion_request_message_content_as_str,
  chat_completion_request_message_tool_calls_as_str,
};
use tui::text::{Span, Spans, Text};

use helix_core::syntax;

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
  pub len_lines: usize,
  pub len_chars: usize,
  pub config_loader: Arc<ArcSwap<syntax::Loader>>,
  // pub markdown: ui::Markdown,
  pub message: ChatMessageType,
}

impl ChatMessageItem {
  pub fn new_chat(
    id: i64,
    message: ChatCompletionRequestMessage,
    config_loader: Arc<ArcSwap<syntax::Loader>>,
  ) -> Self {
    let content =
      chat_completion_request_message_content_as_str(&message).to_string();
    let len_lines = content.lines().count();
    let len_chars = content.chars().count();
    let id = Some(id);
    let message = ChatMessageType::Chat(message.clone());
    Self { id, len_lines, len_chars, config_loader, message }
    // let markdown = ui::Markdown::new(content, config_loader);
  }

  pub fn new_error(
    message: String,
    config_loader: Arc<ArcSwap<syntax::Loader>>,
  ) -> Self {
    let len_lines = message.lines().count();
    let len_chars = message.chars().count();
    let id = None;
    let message = ChatMessageType::Error(message);
    Self { id, len_lines, len_chars, config_loader, message }
  }

  pub fn content(&self) -> &str {
    match &self.message {
      ChatMessageType::Chat(message) => {
        chat_completion_request_message_content_as_str(message)
      },
      ChatMessageType::Error(error) => error,
    }
  }
  pub fn tool_calls(&self) -> Option<Vec<(&str, &str)>> {
    match &self.message {
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
    let (style, header) = match self.message {
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

    // Spans::from(vec![header, markdownText.lines]).into()
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
      let markdown_session = ui::Session::new(
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
