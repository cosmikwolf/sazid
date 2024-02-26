mod markdown;
use helix_core::syntax::Loader;
use helix_view::Theme;
use std::sync::Arc;

use crate::markdown::Markdown;

fn main() {
  let config = Arc::new(Loader::new(test_lang_config()));
  let contents = std::fs::read_to_string("./assets/test.md").unwrap();
  let theme = Theme::default();
  let markdown = Markdown::new(contents, config);

  println!("{:#?}", markdown.parse(Some(&theme)));
}

fn test_lang_config() -> helix_core::syntax::Configuration {
  let default_config = include_bytes!("../assets/languages_test.toml");
  toml::from_str::<helix_core::syntax::Configuration>(
    core::str::from_utf8(default_config).unwrap(),
  )
  .expect("Could not parse built-in languages.toml to valid toml")
}
