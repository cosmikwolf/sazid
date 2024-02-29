use super::Context;
use crate::ui::{self, overlay::overlaid, Picker, Popup};
use async_openai::types::ChatCompletionRequestMessage;
use sazid::app::messages::MessageContainer;
use serde_json::json;
use tui::{
  text::{Span, Spans},
  widgets::Row,
};

use std::{fmt::Write, path::PathBuf};

/// Gets the first language server that is attached to a document which supports a specific feature.
/// If there is no configured language server that supports the feature, this displays a status message.
/// Using this macro in a context where the editor automatically queries the LSP
/// (instead of when the user explicitly does so via a keybind like `gd`)
/// will spam the "No configured language server supports \<feature>" status message confusingly.

struct ChatMessageItem {
  message: ChatCompletionRequestMessage,
}

type MessagePicker = Picker<ChatMessageItem>;

impl ui::menu::Item for ChatMessageItem {
  /// Current working directory.
  type Data = ChatMessageItem;

  fn format(&self, message: &Self::Data) -> Row {
    // The preallocation here will overallocate a few characters since it will account for the
    // URL's scheme, which is not used most of the time since that scheme will be "file://".
    // Those extra chars will be used to avoid allocating when writing the line number (in the
    // common case where it has 5 digits or less, which should be enough for a cast majority
    // of usages).

    let contents = ui::Markdown::new(message, editor.syn_loader.clone());
    contents.parse()
    // Most commonly, this will not allocate, especially on Unix systems where the root prefix
    // is a simple `/` and not `C:\` (with whatever drive letter)
  }

  fn sort_text(&self, data: &Self::Data) -> std::borrow::Cow<str> {
    let label: String = self.format(data).cell_text().collect();
    label.into()
  }

  fn filter_text(&self, data: &Self::Data) -> std::borrow::Cow<str> {
    let label: String = self.format(data).cell_text().collect();
    label.into()
  }
}

pub fn session_messages(cx: &mut Context) {
  let (view, doc) = current!(cx.editor);

  // let language_server =
  //   language_server_with_feature!(cx.editor, doc, LanguageServerFeature::Hover);
  // let pos = doc.position(view.id, language_server.offset_encoding());
  let future = async { Ok(json!("omg wtf!")) };
  // language_server.text_document_hover(doc.identifier(), pos, None).unwrap();

  cx.callback(future, move |editor, compositor, message: Option<String>| {
    if let Some(contents) = message {
      // hover.contents / .range <- used for visualizing

      // fn marked_string_to_markdown(contents: lsp::MarkedString) -> String {
      //   match contents {
      //     lsp::MarkedString::String(contents) => contents,
      //     lsp::MarkedString::LanguageString(string) => {
      //       if string.language == "markdown" {
      //         string.value
      //       } else {
      //         format!("```{}\n{}\n```", string.language, string.value)
      //       }
      //     },
      //   }
      // }
      //
      // let contents = match hover.contents {
      //   lsp::HoverContents::Scalar(contents) => {
      //     marked_string_to_markdown(contents)
      //   },
      //   lsp::HoverContents::Array(contents) => contents
      //     .into_iter()
      //     .map(marked_string_to_markdown)
      //     .collect::<Vec<_>>()
      //     .join("\n\n"),
      //   lsp::HoverContents::Markup(contents) => contents.value,
      // };
      //
      // skip if contents empty

      let contents = ui::Markdown::new(contents, editor.syn_loader.clone());

      let popup = Popup::new("hover", contents).auto_close(false);
      let overlay = overlaid(popup);
      // compositor.replace_or_push("hover", overlay);
      compositor.push(overlay);
    }
  });
}
