#[macro_use]
pub mod macros;

pub mod action;
pub mod app;
pub mod args;
pub mod cli;
pub mod commands;
pub mod components;
pub mod compositor;
pub mod config;
pub mod events;
pub mod handlers;
pub mod job;
pub mod keymap;
pub mod sazid_tui;
pub mod ui;
pub mod utils;

use std::path::Path;

use futures_util::Future;

use ignore::DirEntry;
use url::Url;
#[cfg(windows)]
fn true_color() -> bool {
  true
}

#[cfg(not(windows))]
fn true_color() -> bool {
  if matches!(
    std::env::var("COLORTERM")
      .map(|v| matches!(v.as_str(), "truecolor" | "24bit")),
    Ok(true)
  ) {
    return true;
  }

  match termini::TermInfo::from_env() {
    Ok(t) => {
      t.extended_cap("RGB").is_some()
        || t.extended_cap("Tc").is_some()
        || (t.extended_cap("setrgbf").is_some()
          && t.extended_cap("setrgbb").is_some())
    },
    Err(_) => false,
  }
}

/// Function used for filtering dir entries in the various file pickers.
fn filter_picker_entry(
  entry: &DirEntry,
  root: &Path,
  dedup_symlinks: bool,
) -> bool {
  // We always want to ignore the .git directory, otherwise if
  // `ignore` is turned off, we end up with a lot of noise
  // in our picker.
  if entry.file_name() == ".git" {
    return false;
  }

  // We also ignore symlinks that point inside the current directory
  // if `dedup_links` is enabled.
  if dedup_symlinks && entry.path_is_symlink() {
    return entry
      .path()
      .canonicalize()
      .ok()
      .map_or(false, |path| !path.starts_with(root));
  }

  true
}

/// Opens URL in external program.
fn open_external_url_callback(
  url: Url,
) -> impl Future<Output = Result<job::Callback, anyhow::Error>> + Send + 'static
{
  let commands = open::commands(url.as_str());
  async {
    for cmd in commands {
      let mut command = tokio::process::Command::new(cmd.get_program());
      command.args(cmd.get_args());
      if command.output().await.is_ok() {
        return Ok(job::Callback::Session(Box::new(|_| {})));
      }
    }
    Ok(job::Callback::Session(Box::new(move |editor| {
      editor.set_error("Opening URL in external program failed")
    })))
  }
}