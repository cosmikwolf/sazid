use once_cell::sync::{Lazy, OnceCell};
use serde::{ser::SerializeSeq, Deserialize, Serialize};

// use ahash::RandomState;
// use arc_swap::{ArcSwap, Guard};
// use bitflags::bitflags;
// use hashbrown::raw::RawTable;
// use slotmap::{DefaultKey as LayerId, HopSlotMap};

// use ahash::RandomState;
// use arc_swap::{ArcSwap, Guard};
// use bitflags::bitflags;
// use hashbrown::raw::RawTable;
// use slotmap::{DefaultKey as LayerId, HopSlotMap};

use soft_wrap::SoftWrap;
use std::{
  borrow::Cow,
  cell::RefCell,
  collections::{HashMap, HashSet, VecDeque},
  fmt::{self, Display},
  hash::{Hash, Hasher},
  mem::{replace, transmute},
  path::{Path, PathBuf},
  str::FromStr,
  sync::Arc,
};
use toml;

use crate::snippet::Regex;

fn deserialize_regex<'de, D>(deserializer: D) -> Result<Option<Regex>, D::Error>
where
  D: serde::Deserializer<'de>,
{
  Option::<String>::deserialize(deserializer)?.map(|buf| Regex::new(&buf).map_err(serde::de::Error::custom)).transpose()
}

fn deserialize_lsp_config<'de, D>(deserializer: D) -> Result<Option<serde_json::Value>, D::Error>
where
  D: serde::Deserializer<'de>,
{
  Option::<toml::Value>::deserialize(deserializer)?
    .map(|toml| toml.try_into().map_err(serde::de::Error::custom))
    .transpose()
}

fn deserialize_tab_width<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
  D: serde::Deserializer<'de>,
{
  usize::deserialize(deserializer).and_then(|n| {
    if n > 0 && n <= 16 {
      Ok(n)
    } else {
      Err(serde::de::Error::custom("tab width must be a value from 1 to 16 inclusive"))
    }
  })
}

pub fn deserialize_auto_pairs<'de, D>(deserializer: D) -> Result<Option<AutoPairs>, D::Error>
where
  D: serde::Deserializer<'de>,
{
  Ok(Option::<AutoPairConfig>::deserialize(deserializer)?.and_then(AutoPairConfig::into))
}

fn default_timeout() -> u64 {
  20
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum FileType {
  /// The extension of the file, either the `Path::extension` or the full
  /// filename if the file does not have an extension.
  Extension(String),
  /// The suffix of a file. This is compared to a given file's absolute
  /// path, so it can be used to detect files based on their directories.
  Suffix(String),
}

// largely based on tree-sitter/cli/src/loader.rs
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct LanguageConfiguration {
  #[serde(rename = "name")]
  pub language_id: String, // c-sharp, rust, tsx
  #[serde(rename = "language-id")]
  // see the table under https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocumentItem
  pub language_server_language_id: Option<String>, // csharp, rust, typescriptreact, for the language-server
  pub scope: String,             // source.rust
  pub file_types: Vec<FileType>, // filename extension or ends_with? <Gemfile, rb, etc>
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LanguageServerConfiguration {
  pub command: String,
  #[serde(default)]
  #[serde(skip_serializing_if = "Vec::is_empty")]
  pub args: Vec<String>,
  #[serde(default, skip_serializing_if = "HashMap::is_empty")]
  pub environment: HashMap<String, String>,
  #[serde(default, skip_serializing, deserialize_with = "deserialize_lsp_config")]
  pub config: Option<serde_json::Value>,
  #[serde(default = "default_timeout")]
  pub timeout: u64,
}
// largely based on tree-sitter/cli/src/loader.rs
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct LanguageConfiguration {
  #[serde(rename = "name")]
  pub language_id: String, // c-sharp, rust, tsx
  #[serde(rename = "language-id")]
  // see the table under https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocumentItem
  pub language_server_language_id: Option<String>, // csharp, rust, typescriptreact, for the language-server
  pub scope: String,             // source.rust
  pub file_types: Vec<FileType>, // filename extension or ends_with? <Gemfile, rb, etc>
  #[serde(default)]
  pub shebangs: Vec<String>, // interpreter(s) associated with language
  #[serde(default)]
  pub roots: Vec<String>, // these indicate project roots <.git, Cargo.toml>
  pub comment_token: Option<String>,
  pub text_width: Option<usize>,
  pub soft_wrap: Option<SoftWrap>,

  #[serde(default)]
  pub auto_format: bool,

  #[serde(skip_serializing_if = "Option::is_none")]
  pub formatter: Option<FormatterConfiguration>,

  #[serde(default)]
  pub diagnostic_severity: Severity,

  pub grammar: Option<String>, // tree-sitter grammar name, defaults to language_id

  // content_regex
  #[serde(default, skip_serializing, deserialize_with = "deserialize_regex")]
  pub injection_regex: Option<Regex>,
  // first_line_regex
  //
  #[serde(skip)]
  pub(crate) highlight_config: OnceCell<Option<Arc<HighlightConfiguration>>>,
  // tags_config OnceCell<> https://github.com/tree-sitter/tree-sitter/pull/583
  #[serde(
    default,
    skip_serializing_if = "Vec::is_empty",
    serialize_with = "serialize_lang_features",
    deserialize_with = "deserialize_lang_features"
  )]
  pub language_servers: Vec<LanguageServerFeatures>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub indent: Option<IndentationConfiguration>,

  #[serde(skip)]
  pub(crate) indent_query: OnceCell<Option<Query>>,
  #[serde(skip)]
  pub(crate) textobject_query: OnceCell<Option<TextObjectQuery>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub debugger: Option<DebugAdapterConfig>,

  /// Automatic insertion of pairs to parentheses, brackets,
  /// etc. Defaults to true. Optionally, this can be a list of 2-tuples
  /// to specify a list of characters to pair. This overrides the
  /// global setting.
  #[serde(default, skip_serializing, deserialize_with = "deserialize_auto_pairs")]
  pub auto_pairs: Option<AutoPairs>,

  pub rulers: Option<Vec<u16>>, // if set, override editor's rulers

  /// Hardcoded LSP root directories relative to the workspace root, like `examples` or `tools/fuzz`.
  /// Falling back to the current working directory if none are configured.
  pub workspace_lsp_roots: Option<Vec<PathBuf>>,
  #[serde(default)]
  pub persistent_diagnostic_sources: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub struct SoftWrap {
  /// Soft wrap lines that exceed viewport width. Default to off
  // NOTE: Option on purpose because the struct is shared between language config and global config.
  // By default the option is None so that the language config falls back to the global config unless explicitly set.
  pub enable: Option<bool>,
  /// Maximum space left free at the end of the line.
  /// This space is used to wrap text at word boundaries. If that is not possible within this limit
  /// the word is simply split at the end of the line.
  ///
  /// This is automatically hard-limited to a quarter of the viewport to ensure correct display on small views.
  ///
  /// Default to 20
  pub max_wrap: Option<u16>,
  /// Maximum number of indentation that can be carried over from the previous line when softwrapping.
  /// If a line is indented further then this limit it is rendered at the start of the viewport instead.
  ///
  /// This is automatically hard-limited to a quarter of the viewport to ensure correct display on small views.
  ///
  /// Default to 40
  pub max_indent_retain: Option<u16>,
  /// Indicator placed at the beginning of softwrapped lines
  ///
  /// Defaults to ↪
  pub wrap_indicator: Option<String>,
  /// Softwrap at `text_width` instead of viewport width if it is shorter
  pub wrap_at_text_width: Option<bool>,
}

/// Configuration for auto pairs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, untagged)]
pub enum AutoPairConfig {
  /// Enables or disables auto pairing. False means disabled. True means to use the default pairs.
  Enable(bool),

  /// The mappings of pairs.
  Pairs(HashMap<char, char>),
}

impl Default for AutoPairConfig {
  fn default() -> Self {
    AutoPairConfig::Enable(true)
  }
}

impl From<&AutoPairConfig> for Option<AutoPairs> {
  fn from(auto_pair_config: &AutoPairConfig) -> Self {
    match auto_pair_config {
      AutoPairConfig::Enable(false) => None,
      AutoPairConfig::Enable(true) => Some(AutoPairs::default()),
      AutoPairConfig::Pairs(pairs) => Some(AutoPairs::new(pairs.iter())),
    }
  }
}

impl From<AutoPairConfig> for Option<AutoPairs> {
  fn from(auto_pairs_config: AutoPairConfig) -> Self {
    (&auto_pairs_config).into()
  }
}

impl FromStr for AutoPairConfig {
  type Err = std::str::ParseBoolError;

  // only do bool parsing for runtime setting
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let enable: bool = s.parse()?;
    Ok(AutoPairConfig::Enable(enable))
  }
}
