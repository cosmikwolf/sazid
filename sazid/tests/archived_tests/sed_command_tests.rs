#[cfg(test)]
mod tests {
  use super::*;
  use std::fs::{self, File};
  use std::io::Write;
  use std::io::{self, ErrorKind};

  fn setup_test_file(contents: &str) -> String {
    let test_file = "test.txt";
    let mut file = File::create(test_file).expect("Failed to create test file");
    writeln!(file, "{}", contents).expect("Failed to write to test file");
    test_file.to_string()
  }

  #[test]
  fn test_sed_command_with_no_args() {
    let sed_command = SedCommand;
    let result = sed_command.call(&[]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::InvalidInput);
  }

  #[test]
  fn test_sed_command_with_valid_args() {
    let test_file = setup_test_file("example");

    let sed_command = SedCommand;
    let args = vec!["s/example/replacement/g", test_file.as_str()];
    let result = sed_command.call(&args);

    fs::remove_file(&test_file).expect("Failed to clean up test file");

    assert!(result.is_ok());
    let output = result.unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout), "replacement\n");
  }

  #[test]
  fn test_sed_command_with_invalid_args() {
    let test_file = setup_test_file("example");

    let sed_command = SedCommand;
    let args = vec!["s/example", test_file.as_str()]; // intentionally missing replacement and flags
    let result = sed_command.call(&args);

    fs::remove_file(&test_file).expect("Failed to clean up test file");

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::Other);
  }

  // More tests for different scenarios and edge cases as needed.
}
