use crate::consts::*;
use directories::ProjectDirs;
use tracing_error::ErrorLayer;
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tracing::Level;
use tracing_subscriber::{
    filter::{self, LevelFilter, Targets},
    fmt,
    prelude::*,
    EnvFilter,
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
