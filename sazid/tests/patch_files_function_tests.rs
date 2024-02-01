#[cfg(test)]
mod tests {
  use std::os::unix::fs::PermissionsExt; // Necessary for set_mode
  use std::{
    fs::{self, read_to_string, File},
    path::PathBuf,
  };
  use tempfile::tempdir;

  #[test]
  fn test_apply_patch_file_success() {
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("file_to_patch.txt");
    let patch_path = temp_dir.path().join("patch_file.patch");

    let original_content = "This is the original file content\n";
    let patch_content = "--- file_to_patch.txt\n+++ file_to_patch.txt\n@@ -1,1 +1,1 @@\n-This is the original file content\n+This is the patched file content\n";

    std::fs::write(&file_path, original_content).unwrap();
    std::fs::write(&patch_path, patch_content).unwrap();

    let result = apply_patch_file(file_path.clone(), patch_path);

    assert!(result.is_ok(), "apply_patch_file should succeed");

    let patched_content = read_to_string(file_path).unwrap();
    assert_eq!(patched_content, "This is the patched file content");
  }

  #[test]
  fn test_apply_patch_file_missing_original_file() {
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("nonexistent_file.txt");
    let patch_path = temp_dir.path().join("patch_file.patch");

    let patch_content = "..."; // The actual patch content goes here

    std::fs::write(&patch_path, patch_content).expect("Failed to write patch content");

    let result = apply_patch_file(file_path, patch_path);

    // Assert that an error occurred due to the missing file, not due to parsing
    match result {
      Ok(_) => panic!("apply_patch_file should fail for a nonexistent original file"),
      Err(e) => {
        // Update the assertion here
        assert!(
          e.to_string().contains("error reading original file"),
          "Expected an error related to reading the original file, got: {}",
          e
        );
      },
    }
  }
  #[test]
  fn test_apply_patch_file_invalid_patch_content() {
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("file_to_patch.txt");
    let patch_path = temp_dir.path().join("patch_file.patch");

    let original_content = "This is the original file content\n";
    let patch_content = "invalid patch content";

    std::fs::write(&file_path, original_content).unwrap();
    std::fs::write(&patch_path, patch_content).unwrap();

    let result = apply_patch_file(file_path, patch_path);

    match result {
      Ok(_) => panic!("apply_patch_file should have failed due to invalid patch content"),
      Err(e) => {
        assert!(e.to_string().contains("error parsing patch content"), "Error message did not match expected content");
      },
    }
  }

  #[test]
  fn test_create_patch_file_success() {
    let temp_dir = tempdir().unwrap();
    let patch_path = temp_dir.path().join("patch_file.patch");

    let patch_content = "patch content...";

    let result = create_patch_file(patch_path, patch_content);

    match result {
      Ok(msg) => assert_eq!(msg, "patch file created\n"),
      Err(_) => panic!("create_patch_file should have succeeded"),
    }
  }

  #[test]
  fn test_create_patch_file_error_on_write() {
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("readonly_dir");
    fs::create_dir(&file_path).unwrap();

    // Change permissions to read-only (this is the octal for 444)
    let mut perms = fs::metadata(&file_path).unwrap().permissions();
    perms.set_mode(0o444); // using PermissionsExt trait
    fs::set_permissions(&file_path, perms).unwrap();

    let patch_path = file_path.join("sample.patch");
    let patch_content =
      "--- a/sample.txt\n+++ b/sample.txt\n@@ -1 +1 @@\n-This is the first line.\n+This is an edited line.";

    let result = create_patch_file(patch_path, patch_content);

    // We expect this to fail due to read-only permissions
    match result {
      Ok(_) => panic!("create_patch_file should have failed due to read-only permissions"),
      Err(e) => {
        let error_message = e.to_string();
        assert!(
          error_message.contains("Permission denied (os error 13)"),
          "Expected error due to read-only permissions, got: {}",
          error_message
        );
      },
    }

    // Clean up: set the permissions back to allow writes
    let mut perms = fs::metadata(&file_path).unwrap().permissions();
    perms.set_mode(0o755); // using PermissionsExt trait
    fs::set_permissions(&file_path, perms).unwrap();
    fs::remove_dir_all(&file_path).unwrap(); // remove the temporary directory
  }

  #[test]
  fn test_apply_patch_idempotency() {
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("sample.txt");
    let patch_path = temp_dir.path().join("sample.patch");

    // Create a sample file
    std::fs::write(&file_path, "This is the first line.\n").expect("Failed to write original file content");

    // Create a patch file
    let patch_content =
      "--- a/sample.txt\n+++ b/sample.txt\n@@ -1 +1 @@\n-This is the first line.\n+This is an edited line.\n";
    std::fs::write(&patch_path, patch_content).expect("Failed to write patch content");

    // Apply the patch for the first time
    let first_apply_result = apply_patch_file(file_path.clone(), patch_path.clone());
    assert!(first_apply_result.is_ok(), "Apply first patch should succeed");
    let content_after_first_patch = first_apply_result.unwrap();

    // Apply the patch for the second time
    let second_apply_result = apply_patch_file(file_path.clone(), patch_path);
    assert!(second_apply_result.is_ok(), "Apply second patch should succeed");
    let content_after_second_patch = second_apply_result.unwrap();

    // Verify that the content remains unchanged after applying the patch again
    assert_eq!(
      content_after_first_patch, content_after_second_patch,
      "Content should be unchanged after applying the same patch twice"
    );
  }

  #[test]
  fn test_applying_multiple_patches() {
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("sample.txt");
    let patch1_path = temp_dir.path().join("sample1.patch");
    let patch2_path = temp_dir.path().join("sample2.patch");

    let file_content = "Content before patches.\n";
    std::fs::write(&file_path, file_content).expect("Failed to write file content");

    let patch1_content =
      "--- a/sample.txt\n+++ b/sample.txt\n@@ -1 +1 @@\n-Content before patches.\n+Content after patch 1.\n";
    std::fs::write(&patch1_path, patch1_content).expect("Failed to write patch1 content");

    let apply_result1 = apply_patch_file(file_path.clone(), patch1_path);
    assert!(apply_result1.is_ok(), "Apply first patch should succeed");

    let content_after_patch1 = read_to_string(&file_path).expect("Failed to read file after first patch");

    let patch2_content =
      "--- a/sample.txt\n+++ b/sample.txt\n@@ -1 +1 @@\n-Content after patch 1.\n+Final content after patch 2.\n";
    std::fs::write(&patch2_path, patch2_content).expect("Failed to write patch2 content");

    let apply_result2 = apply_patch_file(file_path.clone(), patch2_path);
    assert!(apply_result2.is_ok(), "Apply second patch should succeed");

    let content_after_patch2 = read_to_string(&file_path).expect("Failed to read file after second patch");

    assert_eq!(
      content_after_patch1, "Content after patch 1.",
      "The content after the first patch did not match expectations."
    );
    assert_eq!(
      content_after_patch2, "Final content after patch 2.",
      "The content after the second patch did not match expectations."
    );
  }

  #[test]
  fn test_invalid_file_path_during_patch_application() {
    let temp_dir = tempdir().unwrap();
    let invalid_file_path = temp_dir.path().join("nonexistent.txt");
    let patch_path = temp_dir.path().join("sample.patch");

    // Create a patch file
    let patch_content = "patch content";
    std::fs::write(&patch_path, patch_content).unwrap();

    // Try to apply the patch to a non-existent file
    let result = apply_patch_file(invalid_file_path, patch_path);

    // Verify that an error is returned
    assert!(result.is_err());
  }

  #[test]
  fn test_create_patch_file_with_empty_content() {
    let temp_dir = tempdir().unwrap();
    let patch_path = temp_dir.path().join("empty.patch");

    // Create a patch file with empty content
    let _result = create_patch_file(patch_path.clone(), "");
    let patch_content = "patch content";
    std::fs::write(patch_path.clone(), patch_content).unwrap();
    let invalid_file_path = PathBuf::from("");
    // Try to apply the patch to a non-existent file
    let result = apply_patch_file(invalid_file_path, patch_path);

    assert!(result.is_err());
  }

  #[test]
  fn test_apply_patch_to_directory() {
    // setup
    let temp_dir = tempdir().unwrap();
    let dir_path = temp_dir.path().join("dir_to_patch");
    std::fs::create_dir_all(&dir_path).unwrap(); // Create a directory
    let patch_path = temp_dir.path().join("patch_file.patch");

    // test patch content
    let patch_content = "--- a\n+++ b\n@@ -1 +1 @@\n-Content before patch.\n+Content after patch.\n";

    // create patch file
    std::fs::write(&patch_path, patch_content).unwrap();

    // action
    let result = apply_patch_file(dir_path, patch_path);

    // verify
    assert!(result.is_err(), "apply_patch_file should fail when the target is a directory");
  }

  #[test]
  fn test_missing_patch_name_argument() {
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("file.txt");
    std::fs::write(&file_path, "Original file content").unwrap();

    // action
    let result = apply_patch_file(file_path, PathBuf::new()); // Empty PathBuf simulates missing patch_name

    // verify
    assert!(result.is_err(), "apply_patch_file should fail when patch_name is missing");
  }

  #[test]
  fn test_error_messages_for_failure_scenarios() {
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("sample.txt");
    let patch_path = temp_dir.path().join("sample.patch");

    // Here we create a file to simulate a real patching scenario
    let file_original_content = "This is the first line.\n";
    std::fs::write(&file_path, file_original_content).unwrap();

    // Sample patch content in GNU Unified Format
    let patch_content =
      "--- a/sample.txt\n+++ b/sample.txt\n@@ -1 +1 @@\n-This is the first line.\n+This is an edited line.\n";

    // We write the patch content to the path from where our function will read
    std::fs::write(&patch_path, patch_content).expect("Failed to write patch content");

    // making the file read-only to trigger a failure scenario in apply_patch_file
    let file = File::create(&file_path).unwrap();
    let mut perms = file.metadata().unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&file_path, perms).unwrap();

    // action - applying our patch
    let result = apply_patch_file(file_path.clone(), patch_path);

    // Since we expect an error due to read-only permissions, we handle the Result accordingly
    match result {
      Ok(_) => panic!("apply_patch_file should have failed due to read-only permissions"),
      Err(e) => {
        // Ensuring that the failure scenario is due to file permission error
        assert!(
          e.to_string().contains("error writing patched file"),
          "Expected error due to read-only file permissions, got: {}",
          e
        );
      },
    }
  }
}
