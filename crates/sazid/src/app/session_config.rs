use std::{
  path::PathBuf,
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_openai::types::ChatCompletionRequestSystemMessage;
use helix_view::editor::{
  get_terminal_provider, CursorShapeConfig, FilePickerConfig, GutterConfig,
  GutterType, LineEndingConfig, LineNumber, LspConfig, PopupBorderConfig,
  TerminalConfig,
};
use serde::{Deserialize, Deserializer, Serialize};

use super::{consts::*, functions::CallableFunction, types::Model};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SessionConfig {
  pub prompt: String,
  pub session_id: String,
  pub session_dir: PathBuf,
  pub available_functions: Vec<CallableFunction>,
  pub accessible_paths: Vec<PathBuf>,
  pub model: Model,
  pub retrieval_augmentation_message_count: Option<i64>,
  pub user: String,
  pub include_functions: bool,
  pub stream_response: bool,
  pub function_result_max_tokens: usize,
  pub response_max_tokens: usize,
  pub file_picker: FilePickerConfig,
  pub lsp: LspConfig,
  pub terminal: Option<TerminalConfig>,
  pub popup_border: PopupBorderConfig,
  pub completion_timeout: Duration,
  pub preview_completion_insert: bool,
  pub completion_trigger_len: u8,
  /// Shape for cursor in each mode
  pub cursor_shape: CursorShapeConfig,
  /// Automatic auto-completion, automatically pop up without user trigger. Defaults to true.
  pub auto_completion: bool,
  /// Padding to keep between the edge of the screen and the cursor when scrolling. Defaults to 5.
  pub scrolloff: usize,
  /// Number of lines to scroll at once. Defaults to 3
  pub scroll_lines: isize,
  /// Mouse support. Defaults to true.
  pub mouse: bool,
  /// Shell to use for shell commands. Defaults to ["cmd", "/C"] on Windows and ["sh", "-c"] otherwise.
  pub shell: Vec<String>,
  /// Line number mode.
  pub line_number: LineNumber,
  /// Highlight the lines cursors are currently on. Defaults to false.
  pub cursorline: bool,
  /// Highlight the columns cursors are currently on. Defaults to false.
  pub cursorcolumn: bool,
  #[serde(deserialize_with = "deserialize_gutter_seq_or_struct")]
  pub gutters: GutterConfig,
  /// Which line ending to choose for new documents. Defaults to `native`. i.e. `crlf` on Windows, otherwise `lf`.
  pub default_line_ending: LineEndingConfig,
}

impl Default for SessionConfig {
  fn default() -> Self {
    SessionConfig {
      prompt: String::new(),
      session_id: Self::generate_session_id(),
      session_dir: PathBuf::new(),
      available_functions: vec![],
      accessible_paths: vec![],
      model: GPT4_TURBO.clone(),
      retrieval_augmentation_message_count: Some(10),
      user: "sazid_user_1234".to_string(),
      function_result_max_tokens: 8192,
      file_picker: FilePickerConfig::default(),
      response_max_tokens: 4095,
      terminal: get_terminal_provider(),
      include_functions: true,
      stream_response: true,
      popup_border: PopupBorderConfig::None,
      lsp: LspConfig::default(),
      completion_timeout: Duration::from_millis(250),
      preview_completion_insert: true,
      completion_trigger_len: 2,
      cursor_shape: CursorShapeConfig::default(),
      auto_completion: false,
      scrolloff: 5,
      scroll_lines: 3,
      mouse: true,
      shell: if cfg!(windows) {
        vec!["cmd".to_owned(), "/C".to_owned()]
      } else {
        vec!["sh".to_owned(), "-c".to_owned()]
      },
      line_number: LineNumber::Absolute,
      cursorline: false,
      cursorcolumn: false,
      gutters: GutterConfig::default(),
      default_line_ending: LineEndingConfig::default(),
    }
  }
}
impl SessionConfig {
  pub fn prompt_message(&self) -> ChatCompletionRequestSystemMessage {
    ChatCompletionRequestSystemMessage {
      content: Some(self.prompt.clone()),
      ..Default::default()
    }
  }

  pub fn generate_session_id() -> String {
    // Get the current time since UNIX_EPOCH in seconds.
    let start = SystemTime::now();
    let since_the_epoch =
      start.duration_since(UNIX_EPOCH).expect("Time went backwards").as_secs();

    // Introduce a delay of 1 second to ensure unique session IDs even if called rapidly.
    std::thread::sleep(std::time::Duration::from_secs(1));

    // Convert the duration to a String and return.
    since_the_epoch.to_string()
  }
}

fn deserialize_gutter_seq_or_struct<'de, D>(
  deserializer: D,
) -> Result<GutterConfig, D::Error>
where
  D: Deserializer<'de>,
{
  struct GutterVisitor;

  impl<'de> serde::de::Visitor<'de> for GutterVisitor {
    type Value = GutterConfig;

    fn expecting(
      &self,
      formatter: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
      write!(
        formatter,
        "an array of gutter names or a detailed gutter configuration"
      )
    }

    fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
    where
      S: serde::de::SeqAccess<'de>,
    {
      let mut gutters = Vec::new();
      while let Some(gutter) = seq.next_element::<String>()? {
        gutters
          .push(gutter.parse::<GutterType>().map_err(serde::de::Error::custom)?)
      }

      Ok(gutters.into())
    }

    fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
    where
      M: serde::de::MapAccess<'de>,
    {
      let deserializer = serde::de::value::MapAccessDeserializer::new(map);
      Deserialize::deserialize(deserializer)
    }
  }

  deserializer.deserialize_any(GutterVisitor)
}
