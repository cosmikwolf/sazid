#[cfg(test)]
mod tests {
  use super::*;
  use sazid::app::functions::patch_files_function::{apply_patch_file, create_patch_file};
  use std::{fs::read_to_string, path::PathBuf};
  use tempfile::tempdir;

  #[test]
  fn test_apply_patch_file_success() {
    // setup
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("file_to_patch.txt");
    let patch_path = temp_dir.path().join("patch_file.patch");

    // test content and patch
    let original_content = "This is the original file content\n";
    let patch_content = "--- file_to_patch.txt\n+++ file_to_patch.txt\n@@ -1,1 +1,1 @@\n-This is the original file content\n+This is the patched file content\n";

    // create original file
    std::fs::write(&file_path, original_content).unwrap();

    // create patch file with valid content
    std::fs::write(&patch_path, patch_content).unwrap();

    // action
    let result = apply_patch_file(file_path.clone(), patch_path);

    // verify
    assert!(result.is_ok(), "apply_patch_file should succeed");
    // we expect the file content to be updated to "This is the patched file content"
    let patched_content = std::fs::read_to_string(file_path).unwrap();
    assert_eq!(patched_content, "This is the patched file content");
  }

  #[test]
  fn test_apply_patch_file_missing_original_file() {
    // setup
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("nonexistent_file.txt");
    let patch_path = temp_dir.path().join("patch_file.patch");

    // test patch content
    let patch_content = "patch content...";

    // create patch file
    std::fs::write(&patch_path, patch_content).unwrap();

    // action
    let result = apply_patch_file(file_path, patch_path);

    // verify
    assert!(result.is_err(), "apply_patch_file should fail for a nonexistent original file");
  }

  #[test]
  fn test_apply_patch_file_invalid_patch_content() {
    // setup
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("file_to_patch.txt");
    let patch_path = temp_dir.path().join("patch_file.patch");

    // test content and invalid patch
    let original_content = "This is the original file content\n";
    let patch_content = "invalid patch content";

    // create original file
    std::fs::write(&file_path, original_content).unwrap();

    // create patch file with invalid content
    std::fs::write(&patch_path, patch_content).unwrap();

    // action
    let result = apply_patch_file(file_path, patch_path);

    // verify
    assert!(result.is_err(), "apply_patch_file should fail for invalid patch content");
  }

  #[test]
  fn test_create_patch_file_success() {
    // setup
    let temp_dir = tempdir().unwrap();
    let patch_path = temp_dir.path().join("patch_file.patch");

    // test patch content
    let patch_content = "patch content...";

    // action
    let result = create_patch_file(patch_path, patch_content);

    // verify
    assert!(result.is_ok(), "create_patch_file should succeed");
    assert_eq!(result.unwrap(), "patch file created\n");
  }

  #[test]
  fn test_create_patch_file_error_on_write() {
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("sample.txt");
    let patch_path = temp_dir.path().join("sample.patch");
    let patch_content =
      "--- a/sample.txt\n+++ b/sample.txt\n@@ -1 +1 @@\n-This is the first line.\n+This is an edited line.";
    std::fs::write(&file_path, "This is the first line.\n").unwrap();

    // Apply the patch for the first time
    apply_patch_file(file_path.clone(), patch_path.clone()).unwrap();
    // action
    let result = create_patch_file(patch_path, patch_content);

    // verify
    assert!(result.is_err(), "create_patch_file should fail when unable to create patch file");
  }

  #[test]
  fn test_apply_patch_idempotency() {
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("sample.txt");
    let patch_path = temp_dir.path().join("sample.patch");

    // Create a sample file
    std::fs::write(&file_path, "This is the first line.\n").unwrap();

    // Create a patch file
    let patch_content =
      "--- a/sample.txt\n+++ b/sample.txt\n@@ -1 +1 @@\n-This is the first line.\n+This is an edited line.";
    std::fs::write(&patch_path, patch_content).unwrap();

    // Apply the patch for the first time
    apply_patch_file(file_path.clone(), patch_path.clone()).unwrap();
    let content_after_first_patch = read_to_string(&file_path).unwrap();

    // Apply the patch for the second time
    apply_patch_file(file_path.clone(), patch_path).unwrap();
    let content_after_second_patch = read_to_string(&file_path).unwrap();

    // Verify that the content remains unchanged after applying the patch again
    assert_eq!(content_after_first_patch, content_after_second_patch);
  }

  #[test]
  fn test_applying_multiple_patches() {
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("sample.txt");
    let patch1_path = temp_dir.path().join("sample1.patch");
    let patch2_path = temp_dir.path().join("sample2.patch");
    let patch1_content = "--- a/sample.txt\n+++ b/sample.txt\n@@ -1 +1 @@+Content after patch 1.\n";
    std::fs::write(&file_path, "Content before patches.\n").unwrap();
    let patch2_content =
      "--- a/sample.txt\n+++ b/sample.txt\n@@ -1 +1 @@\n-Content after patch 1.\n+Final content after patch 2.\n";
    std::fs::write(&file_path, "Content before patches.\n").unwrap();

    // Apply patch 1
    apply_patch_file(file_path.clone(), patch1_path).unwrap();
    std::fs::write(&patch1_path, patch1_content).unwrap();

    // Create patch 2
    let patch2_content =
      "--- a/sample.txt\n+++ b/sample.txt\n@@ -1 +1 @@\n-Content after patch 1.\n+Final content after patch 2.\n";
    std::fs::write(&patch2_path, patch2_content).unwrap();

    // Apply patch 1
    apply_patch_file(file_path.clone(), patch1_path).unwrap();
    let content_after_patch1 = read_to_string(&file_path).unwrap();

    // Apply patch 2
    apply_patch_file(file_path.clone(), patch2_path).unwrap();
    let content_after_patch2 = read_to_string(&file_path).unwrap();

    // Verify that the content is as expected after applying both patches
    assert_eq!(content_after_patch1, "Content after patch 1.\n");
    assert_eq!(content_after_patch2, "Final content after patch 2.\n");
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
    let result = create_patch_file(patch_path, "");
    let patch_content = "patch content";
    std::fs::write(&patch_path, patch_content).unwrap();

    // Try to apply the patch to a non-existent file
    let result = apply_patch_file(invalid_file_path, patch_path);
  }
}

