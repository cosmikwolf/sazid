use std::path::Path;

pub mod interface;
pub mod query;
pub mod status_message;
pub mod symbol_types;
pub mod tool_impl;
pub mod workspace;
pub mod workspace_file;

use lsp_types as lsp;
use ropey::Rope;

fn position_gt(pos1: lsp::Position, pos2: lsp::Position) -> bool {
  if pos1.line > pos2.line {
    true
  } else {
    pos1.line == pos2.line && pos1.character > pos2.character
  }
}

fn get_file_range_contents(file_path: &Path, range: lsp::Range) -> anyhow::Result<String> {
  let source_code = std::fs::read_to_string(file_path)?;
  if range.start == range.end {
    return Ok(String::new());
  }
  let source_code = source_code
    .lines()
    .skip(range.start.line as usize)
    .take((range.end.line - range.start.line) as usize + 1)
    .enumerate()
    .map(|(i, line)| {
      if i == 0 {
        line.chars().skip(range.start.character as usize).collect()
      } else if i == (range.end.line - range.start.line) as usize {
        line.chars().take(range.end.character as usize).collect()
      } else {
        line.to_string()
      }
    })
    .collect::<Vec<_>>()
    .join("\n");
  Ok(source_code)
}

pub fn replace_file_range_contents(
  file_path: &Path,
  range: lsp::Range,
  contents: String,
) -> anyhow::Result<String> {
  let mut rope = Rope::from_reader(std::fs::File::open(file_path)?)?;

  let start_char = rope.line_to_char(range.start.line as usize) + range.start.character as usize;
  let end_char = rope.line_to_char(range.end.line as usize) + range.end.character as usize;

  rope.remove(start_char..end_char);
  rope.insert(start_char, &contents);

  let new_contents = rope.to_string();
  std::fs::write(file_path, &new_contents)?;

  Ok(new_contents)
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::fs::File;
  use std::io::Write;
  use tempfile::tempdir;

  #[test]
  fn test_replace_file_range_contents() {
    // Create a temporary directory and file for testing
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    let mut file = File::create(&file_path).unwrap();
    writeln!(file, "line 1\nline 2\nline 3\nline 4\nline 5").unwrap();

    // Test replacing content within multiple lines
    let range = lsp::Range {
      start: lsp::Position { line: 1, character: 2 },
      end: lsp::Position { line: 2, character: 5 },
    };
    let contents = "new content".to_string();
    let result = replace_file_range_contents(&file_path, range, contents.clone()).unwrap();
    let expected_result = "line 1\nlinew content\nline 5".to_string();
    assert_eq!(result, expected_result);

    // Check the contents of the file
    let file_contents = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(file_contents, expected_result);

    // Test replacing content within a single line
    let range = lsp::Range {
      start: lsp::Position { line: 0, character: 2 },
      end: lsp::Position { line: 0, character: 5 },
    };
    let contents = "new".to_string();
    let result = replace_file_range_contents(&file_path, range, contents).unwrap();
    let expected_result = "linew 1\nline 2\nline 3\nline 4\nline 5".to_string();
    assert_eq!(result, expected_result);

    // Test replacing content from the beginning of the file to the middle of a line
    let range = lsp::Range {
      start: lsp::Position { line: 0, character: 0 },
      end: lsp::Position { line: 1, character: 3 },
    };
    let contents = "start".to_string();
    let result = replace_file_range_contents(&file_path, range, contents).unwrap();
    let expected_result = "starte 2\nline 3\nline 4\nline 5".to_string();
    assert_eq!(result, expected_result);

    // Test replacing the entire content of the file
    let range = lsp::Range {
      start: lsp::Position { line: 0, character: 0 },
      end: lsp::Position { line: 4, character: 6 },
    };
    let contents = "new file content".to_string();
    let result = replace_file_range_contents(&file_path, range, contents).unwrap();
    let expected_result = "new file content".to_string();
    assert_eq!(result, expected_result);

    // Test inserting content at a specific position
    let range = lsp::Range {
      start: lsp::Position { line: 0, character: 8 },
      end: lsp::Position { line: 0, character: 8 },
    };
    let contents = "inserted ".to_string();
    let result = replace_file_range_contents(&file_path, range, contents).unwrap();
    let expected_result = "new fileinserted  content".to_string();
    assert_eq!(result, expected_result);
  }
}
