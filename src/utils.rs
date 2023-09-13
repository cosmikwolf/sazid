use crate::components::consts::*;
use crate::tui::Tui;
use directories::ProjectDirs;
use tracing_error::ErrorLayer;
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use tracing::Level;
use tracing_subscriber::{
    filter::{LevelFilter, Targets},
    fmt,
};

use color_eyre::eyre::Result;
use tracing::error;
use tracing_subscriber::{
  self, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, Layer,
};

// list all sessions in the sessions directory
pub fn list_sessions() -> io::Result<Vec<PathBuf>> {
    ensure_directory_exists(SESSIONS_DIR)?;
    let mut sessions: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(SESSIONS_DIR)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            sessions.push(path);
        }
    }
    Ok(sessions)
}
pub fn ensure_directory_exists(dir: &str) -> io::Result<()> {
    let dir_path = Path::new(dir);
    if !dir_path.exists() {
        fs::create_dir_all(dir_path)?;
    }
    Ok(())
}

fn project_directory() -> Option<ProjectDirs> {
    ProjectDirs::from(
        "com",
        "zetaohm",
        PROJECT_NAME.clone().to_lowercase().as_str(),
    )
}

pub fn get_data_dir() -> PathBuf {
    let directory = if let Some(s) = DATA_FOLDER.clone() {
        s
    } else if let Some(proj_dirs) = project_directory() {
        proj_dirs.data_local_dir().to_path_buf()
    } else {
        PathBuf::from(".").join(".data")
    };
    directory
}

pub fn initialize_tracing() -> Result<(), Box<dyn std::error::Error>> {
    let directory = get_data_dir();
    std::fs::create_dir_all(directory.clone())?;
    let log_path = directory.join(LOG_FILE.clone());
    println!("log_path: {:?}", log_path);
    // create file at log_path, ensuring the parent directory exists
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(log_path)?;

    let log_subscriber = fmt::layer()
        // Use a more compact, abbreviated log format
        .compact()
        // Display source code file paths
        .with_file(true)
        // Display source code line numbers
        .with_line_number(true)
        // Display the thread ID an event was recorded on
        .with_thread_ids(true)
        // Don't display the event's target (module path)
        .with_target(true)
        // Build the subscriber
        .with_writer(log_file)
        .pretty()
        .with_filter(
            Targets::new()
                // Enable the `INFO` level for anything in `my_crate`
                .with_target("sazid", Level::TRACE)
                // Enable the `DEBUG` level for a specific module.
                .with_target("tokio", LevelFilter::OFF)
                .with_target("runtime", LevelFilter::OFF),
        );

    let console_layer = console_subscriber::ConsoleLayer::builder()
        .with_default_env()
        .spawn()
        .with_filter( LevelFilter::TRACE);
        // .with_filter(Targets::default().with_target("sazid", Level::TRACE));

    tracing_subscriber::registry()
        .with(console_layer)
        .with(log_subscriber)
        .with(ErrorLayer::default())
        .init();

    // This event will only be seen by the debug log file layer:
    tracing::debug!("this is a message, and part of a system of messages");

    // This event will be seen by both the stdout log layer *and*
    // the debug log file layer, but not by the metrics layer.
    Ok(())
}


#[macro_export]
macro_rules! trace_dbg {
    (target: $target:expr, level: $level:expr, $ex:expr) => {{
        match $ex {
            value => {
                tracing::event!(target: $target, $level, ?value, stringify!($ex));
                value
            }
        }
    }};
    (level: $level:expr, $ex:expr) => {
        trace_dbg!(target: module_path!(), level: $level, $ex)
    };
    (target: $target:expr, $ex:expr) => {
        trace_dbg!(target: $target, level: tracing::Level::DEBUG, $ex)
    };
    ($ex:expr) => {
        trace_dbg!(level: tracing::Level::DEBUG, $ex)
    };
}

pub fn initialize_panic_handler() -> Result<()> {
    let (panic_hook, eyre_hook) = color_eyre::config::HookBuilder::default()
      .panic_section(format!("This is a bug. Consider reporting it at {}", env!("CARGO_PKG_REPOSITORY")))
      .display_location_section(true)
      .display_env_section(true)
      .issue_url(concat!(env!("CARGO_PKG_REPOSITORY"), "/issues/new"))
      .add_issue_metadata("version", env!("CARGO_PKG_VERSION"))
      .add_issue_metadata("os", std::env::consts::OS)
      .add_issue_metadata("arch", std::env::consts::ARCH)
      .into_hooks();
    eyre_hook.install()?;
    std::panic::set_hook(Box::new(move |panic_info| {
      if let Ok(t) = Tui::new() {
        if let Err(r) = t.exit() {
          error!("Unable to exit Terminal: {:?}", r);
        }
      }
  
      let msg = format!("{}", panic_hook.panic_report(panic_info));
      eprintln!("{}", msg);
      log::error!("Error: {}", strip_ansi_escapes::strip_str(msg));
  
      use human_panic::{handle_dump, print_msg, Metadata};
      let meta = Metadata {
        version: env!("CARGO_PKG_VERSION").into(),
        name: env!("CARGO_PKG_NAME").into(),
        authors: env!("CARGO_PKG_AUTHORS").replace(':', ", ").into(),
        homepage: env!("CARGO_PKG_HOMEPAGE").into(),
      };
  
      let file_path = handle_dump(&meta, panic_info);
      print_msg(file_path, &meta).expect("human-panic: printing error message to console failed");
  
      // Better Panic. Only enabled *when* debugging.
      #[cfg(debug_assertions)]
      {
        better_panic::Settings::auto()
          .most_recent_first(false)
          .lineno_suffix(true)
          .verbosity(better_panic::Verbosity::Full)
          .create_panic_handler()(panic_info);
      }
  
      std::process::exit(libc::EXIT_FAILURE);
    }));
    Ok(())
  }
    
  pub fn get_config_dir() -> PathBuf {
    let directory = if let Some(s) = CONFIG_FOLDER.clone() {
      s
    } else if let Some(proj_dirs) = project_directory() {
      proj_dirs.config_local_dir().to_path_buf()
    } else {
      PathBuf::from(".").join(".config")
    };
    directory
  }

pub fn version() -> String {
  let author = clap::crate_authors!();

  let commit_hash = GIT_COMMIT_HASH.clone();

  // let current_exe_path = PathBuf::from(clap::crate_name!()).display().to_string();
  let config_dir_path = get_config_dir().display().to_string();
  let data_dir_path = get_data_dir().display().to_string();

  format!(
    "\
{commit_hash}

Authors: {author}

Config directory: {config_dir_path}
Data directory: {data_dir_path}"
  )
}