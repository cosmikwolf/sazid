use std::path::Path;

pub mod interface;
pub mod query;
pub mod status_message;
pub mod symbol_types;
pub mod tool_impl;
pub mod workspace;
pub mod workspace_file;

use lsp_types as lsp;

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
