use anyhow::{anyhow, Result};
use std::path::Path;
use std::{fmt, usize};
use tree_sitter::{Language, Point};

#[derive(Debug, Default)]
pub struct Stats {
  pub successful_parses: usize,
  pub total_parses: usize,
}

impl fmt::Display for Stats {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    writeln!(
      f,
      "Total parses: {}; successful parses: {}; failed parses: {}; success percentage: {:.2}%",
      self.total_parses,
      self.successful_parses,
      self.total_parses - self.successful_parses,
      (self.successful_parses as f64) / (self.total_parses as f64) * 100.0
    )
  }
}

#[derive(Copy, Clone)]
pub enum ParseOutput {
  Normal,
  Quiet,
  Xml,
  Tree,
}

pub struct ParseFileOptions<'a> {
  pub language: Language,
  pub path: &'a Path,
  pub edits: &'a [&'a str],
  pub max_path_length: usize,
  pub output: ParseOutput,
  pub print_time: bool,
  pub timeout: u64,
  pub debug: bool,
}

pub fn offset_for_position(input: &[u8], position: Point) -> Result<usize> {
  let mut row = 0;
  let mut offset = 0;
  let mut iter = memchr::memchr_iter(b'\n', input);
  loop {
    if let Some(pos) = iter.next() {
      if row < position.row {
        row += 1;
        offset = pos;
        continue;
      }
    }
    offset += 1;
    break;
  }
  if position.row - row > 0 {
    return Err(anyhow!("Failed to address a row: {}", position.row));
  }
  if let Some(pos) = iter.next() {
    if (pos - offset < position.column) || (input[offset] == b'\n' && position.column > 0) {
      return Err(anyhow!("Failed to address a column: {}", position.column));
    };
  } else if input.len() - offset < position.column {
    return Err(anyhow!("Failed to address a column over the end"));
  }
  Ok(offset + position.column)
}

// fn parse_edit_flag(source_code: &Vec<u8>, flag: &str) -> Result<Edit> {
//   let error = || {
//     anyhow!(
//       concat!(
//         "Invalid edit string '{}'. ",
//         "Edit strings must match the pattern '<START_BYTE_OR_POSITION> <REMOVED_LENGTH> <NEW_TEXT>'"
//       ),
//       flag
//     )
//   };
//
//   // Three whitespace-separated parts:
//   // * edit position
//   // * deleted length
//   // * inserted text
//   let mut parts = flag.split(' ');
//   let position = parts.next().ok_or_else(error)?;
//   let deleted_length = parts.next().ok_or_else(error)?;
//   let inserted_text = parts.collect::<Vec<_>>().join(" ").into_bytes();
//
//   // Position can either be a byte_offset or row,column pair, separated by a comma
//   let position = if position == "$" {
//     source_code.len()
//   } else if position.contains(',') {
//     let mut parts = position.split(',');
//     let row = parts.next().ok_or_else(error)?;
//     let row = row.parse::<usize>().map_err(|_| error())?;
//     let column = parts.next().ok_or_else(error)?;
//     let column = column.parse::<usize>().map_err(|_| error())?;
//     offset_for_position(source_code, Point { row, column })?
//   } else {
//     position.parse::<usize>().map_err(|_| error())?
//   };
//
//   // Deleted length must be a byte count.
//   let deleted_length = deleted_length.parse::<usize>().map_err(|_| error())?;
//
//   Ok(Edit { position, deleted_length, inserted_text })
// }
