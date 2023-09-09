use std::{time::{SystemTime, UNIX_EPOCH}, path::Path, io, fs};

pub fn generate_session_id() -> String {
    // Get the current time since UNIX_EPOCH in seconds.
    let start = SystemTime::now();
    let since_the_epoch = start.duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();

    // Introduce a delay of 1 second to ensure unique session IDs even if called rapidly.
    std::thread::sleep(std::time::Duration::from_secs(1));

    // Convert the duration to a String and return.
    since_the_epoch.to_string()
}

pub fn ensure_directory_exists(dir: &str) -> io::Result<()> {
    let dir_path = Path::new(dir);
    if !dir_path.exists() {
        fs::create_dir_all(dir_path)?;
    }
    Ok(())
}
