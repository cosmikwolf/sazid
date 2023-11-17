#[cfg(test)]
mod tests {
  use super::*;
  use sazid::app::functions::patch_files_function::{apply_patch_file, create_patch_file};
  use std::path::PathBuf;
  use tempfile::tempdir;

  #[test]
  fn test_apply_patch_file_success() {
    // setup
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("file_to_patch.txt");
    let patch_path = temp_dir.path().join("patch_file.patch");

    // test content and patch
    let original_content = "This is the original file content\n";
    let patch_content = "patch content...";

    // create original file
    std::fs::write(&file_path, original_content).unwrap();

    // create patch file
    std::fs::write(&patch_path, patch_content).unwrap();

    // action
    let result = apply_patch_file(file_path, patch_path);

    // verify
    assert!(result.is_ok(), "apply_patch_file should succeed");
    assert_eq!(result.unwrap(), "Patch applied successfully");
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
    // no setup required, as we will pass an invalid path
    let patch_path = PathBuf::from("/invalid/path/patch_file.patch");

    // test patch content
    let patch_content = "patch content...";

    // action
    let result = create_patch_file(patch_path, patch_content);

    // verify
    assert!(result.is_err(), "create_patch_file should fail when unable to create patch file");
  }
}
