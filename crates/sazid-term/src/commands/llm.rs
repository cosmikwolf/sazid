use super::Context;
use crate::{
  compositor::{self, Compositor},
  job::Callback,
  ui::{
    self,
    markdownmenu::MarkdownMenu,
    overlay::overlaid,
    textbox::{Textbox, TextboxEvent},
    Picker, Popup, PromptEvent, Text,
  },
};
use arc_swap::ArcSwap;
use async_openai::types::{
  ChatCompletionRequestAssistantMessage, ChatCompletionRequestMessage,
  ChatCompletionRequestSystemMessage, ChatCompletionRequestUserMessage,
  ChatCompletionRequestUserMessageContent, Role,
};
use futures_util::{stream::FuturesUnordered, Future};
use helix_lsp::block_on;
use helix_view::{
  theme::{Color, Style},
  Editor, Theme,
};
use sazid::{
  app::messages::{
    get_chat_message_text, get_chat_message_type_string, ChatMessage,
    MessageContainer,
  },
  components::session::Session,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tui::{
  text::{Span, Spans},
  widgets::Row,
};

use helix_core::{
  fuzzy::MATCHER,
  syntax::{self, LanguageServerFeature},
};

use std::{fmt::Write, path::PathBuf, sync::Arc};

/// Gets the first language server that is attached to a document which supports a specific feature.
/// If there is no configured language server that supports the feature, this displays a status message.
/// Using this macro in a context where the editor automatically queries the LSP
/// (instead of when the user explicitly does so via a keybind like `gd`)
/// will spam the "No configured language server supports \<feature>" status message confusingly.

pub struct ChatMessageItem {
  pub markdown: ui::Markdown,
  pub message: ChatCompletionRequestMessage,
}

type MessagePicker = Picker<ChatMessageItem>;

impl ui::markdownmenu::MarkdownItem for ChatMessageItem {
  /// Current working directory.
  type Data = String;

  fn format(
    &self,
    data: &Self::Data,
    theme: Option<&Theme>,
  ) -> tui::text::Text {
    // The preallocation here will overallocate a few characters since it will account for the
    // URL's scheme, which is not used most of the time since that scheme will be "file://".
    // Those extra chars will be used to avoid allocating when writing the line number (in the
    // common case where it has 5 digits or less, which should be enough for a cast majority
    // of usages).
    // data.to_string().into()
    // let contents = ui::Markdown::new(message, editor.syn_loader.clone());
    // contents.parse()
    // Most commonly, this will not allocate, especially on Unix systems where the root prefix
    // is a simple `/` and not `C:\` (with whatever drive letter)
    // log::debug!("string: {}", self.content);

    let style = match self.message {
      ChatCompletionRequestMessage::System(_) => Style::default()
        .fg(Color::Magenta)
        .add_modifier(helix_view::theme::Modifier::ITALIC),
      ChatCompletionRequestMessage::User(_) => Style::default()
        .fg(Color::Green)
        .add_modifier(helix_view::theme::Modifier::ITALIC),
      ChatCompletionRequestMessage::Assistant(_) => Style::default()
        .fg(Color::Blue)
        .add_modifier(helix_view::theme::Modifier::ITALIC),
      ChatCompletionRequestMessage::Tool(_) => Style::default()
        .fg(Color::Yellow)
        .add_modifier(helix_view::theme::Modifier::ITALIC),
      ChatCompletionRequestMessage::Function(_) => Style::default()
        .fg(Color::LightYellow)
        .add_modifier(helix_view::theme::Modifier::ITALIC),
    };
    let header = Spans::from(vec![Span::styled(
      get_chat_message_type_string(&self.message),
      style,
    )]);
    let mut lines = vec![header];
    lines.extend(self.markdown.parse(theme).lines);
    // lines.insert(0, header);
    lines.into()

    // Spans::from(vec![header, markdownText.lines]).into()
  }
}

pub fn session_messages(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);

  let content1 = r#"## **Table of Contents**

- [**Features**](#features)
- [**Getting Started**](#getting-started)
  - [**Prerequisites**](#prerequisites)
  - [**Installation**](#installation)
- [**Usage**](#usage)
- [**Contributing**](#contributing)
- [**License**](#license)
- [**Acknowledgements**](#acknowledgements)"#;

  let content2 = r#"### **Installation**

- configure your OPENAI_API_KEY env variable

### Vector DB Setup

#### Start the database service

- docker-compose up -d
"#;
  let content3 = r#"## Usage

Sazid is currently a work in progress.

It can currently be used as an LLM interface with function calls that allow GPT to:

- Read files
- Write files
- pcre2grep files

There is also disabled code that enables GPT to use sed and a custom function to modify files directly, but I have found that GPT consistently makes annoying mistakes that are extremely frustrating to resolve when it is forced to use regular expressions and line numbers to modify files.
"#;

  let fixture_msg1 = ChatCompletionRequestSystemMessage {
    content: content1.to_string(),
    role: Role::System,
    name: Some("sazid".to_string()),
  };

  let fixture_msg2 = ChatCompletionRequestUserMessage {
    content: ChatCompletionRequestUserMessageContent::Text(
      "omg plz help me with things... tell me the things...".to_string(),
    ),
    role: Role::Assistant,
    name: Some("sazid".to_string()),
  };

  let fixture_msg3 = ChatCompletionRequestAssistantMessage {
    content: Some(content1.to_string()),
    role: Role::User,
    name: Some("sazid".to_string()),
    tool_calls: None,
    function_call: None,
  };

  cx.session.add_message(ChatMessage::System(fixture_msg1));
  cx.session.add_message(ChatMessage::User(fixture_msg2));
  cx.session.add_message(ChatMessage::Assistant(fixture_msg3));

  let messages_fut = futures_util::future::ready(
    cx.session
      .messages
      .clone()
      .iter()
      .map(|message| ChatMessageItem {
        markdown: ui::Markdown::new(
          message.to_string(),
          cx.editor.syn_loader.clone(),
        ),
        message: message.message.clone(),
      })
      .collect::<Vec<_>>(),
  );

  let callback_fn = |_editor: &mut Editor,
                     _item: Option<&ChatMessageItem>,
                     _event: ui::PromptEvent| {};

  let session_callback =
    |context: &mut compositor::Context,
     message: &ChatMessageItem,
     action: helix_view::editor::Action| {};

  cx.jobs.callback(async move {
    // let mut messages = Vec::new();
    // // TODO if one symbol request errors, all other requests are discarded (even if they're valid)
    // while let Some(mut msgs) = messages_fut.try_next().await? {
    //   messages.append(&mut msgs);
    // }
    let messages = messages_fut.await;
    let call = move |editor: &mut Editor, compositor: &mut Compositor| {
      let editor_data = get_chat_message_text(&messages[0].message);
      // let markdown_message = MarkdownMenu::new(
      //   messages.clone(),
      //   editor_data.clone(),
      //   callback_fn,
      //   editor.syn_loader.clone(),
      //   Some(editor.theme.clone()),
      // );
      //
      let markdown_session = ui::Session::new(
        messages,
        Some(editor.theme.clone()),
        editor_data,
        session_callback,
      );
      let textbox = create_textbox(editor, "".to_string(), None);
      compositor.replace_or_push("markdown text", overlaid(markdown_session));
      // compositor.push(Box::new(textbox))
      // compositor.replace_or_push("textbox test", Popup::new("textbox", textbox))
    };

    Ok(Callback::EditorCompositor(Box::new(call)))
    // Ok(Callback::EditorCompositor(textbox))
  });
}

fn create_textbox(
  editor: &Editor,
  prefill: String,
  language_server_id: Option<usize>,
) -> Textbox {
  Textbox::new(
    "textbox:".into(),
    None,
    ui::completers::none,
    move |cx: &mut compositor::Context, input: &str, event: TextboxEvent| {
      if event != TextboxEvent::Validate {
        return;
      }
      let (view, doc) = current!(cx.editor);

      let Some(language_server) = doc
        .language_servers_with_feature(LanguageServerFeature::RenameSymbol)
        .find(|ls| language_server_id.map_or(true, |id| id == ls.id()))
      else {
        cx.editor
          .set_error("No configured language server supports symbol renaming");
        return;
      };

      let offset_encoding = language_server.offset_encoding();
      let pos = doc.position(view.id, offset_encoding);
      let future = language_server
        .rename_symbol(doc.identifier(), pos, input.to_string())
        .unwrap();

      match block_on(future) {
        Ok(edits) => {
          let _ = cx.editor.apply_workspace_edit(offset_encoding, &edits);
        },
        Err(err) => cx.editor.set_error(err.to_string()),
      }
    },
  )
  .with_line(prefill, editor)
}
