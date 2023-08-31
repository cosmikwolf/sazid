use std::time::{SystemTime, UNIX_EPOCH};

pub fn generate_session_id() -> String {
    // Get the current time since UNIX_EPOCH in seconds.
    let start = SystemTime::now();
    let since_the_epoch = start.duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();

    // Convert the duration to a String and return.
    since_the_epoch.to_string()
}