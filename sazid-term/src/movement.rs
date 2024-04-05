use helix_core::{
  doc_formatter::TextFormat,
  graphemes::{nth_next_grapheme_boundary, nth_prev_grapheme_boundary},
  movement::{Direction, Movement},
  text_annotations::TextAnnotations,
  Range, RopeSlice,
};
use helix_view::graphics::Rect;

use crate::commands::ChatMessageItem;

fn translate_pos_to_char_index(
  text: RopeSlice<'_>,
  area: Rect,
  pos: helix_core::Position,
) -> usize {
  let mut pos = pos;
  pos.row = pos.row.saturating_sub(area.top() as usize);
  pos.col = pos.col.saturating_sub(area.left() as usize);
  let row_start_index = text.line_to_char(pos.row);
  let col = pos.col;
  row_start_index + col
}

pub fn translate_char_index_to_pos(
  text: RopeSlice<'_>,
  // area: Rect,
  index: usize,
) -> helix_core::Position {
  log::info!("translate_char_index_to_pos: index: {}", index);
  let row = text.char_to_line(index);
  let row_start_index = text.line_to_char(row);
  let col = index - row_start_index;
  helix_core::Position::new(
    // row + area.top() as usize,
    // col + area.left() as usize,
    row, col,
  )
}

pub fn session_move_horizontally(
  message: Vec<ChatMessageItem>,
  range: Range,
  dir: Direction,
  count: usize,
  behaviour: Movement,
  _: &TextFormat,
  _: &mut TextAnnotations,
) -> Range {
  let pos = range.cursor(slice);

  // Compute the new position.
  let new_pos = match dir {
    Direction::Forward => pos + count,
    Direction::Backward => pos - count,
  }
  .clamp(0, slice.len_chars());

  // Compute the final new range.
  range.put_cursor(slice, new_pos, behaviour == Movement::Extend)
}

pub fn session_move_vertically(
  message: Vec<ChatMessageItem>,
  range: Range,
  dir: Direction,
  count: usize,
  behaviour: Movement,
  _: &TextFormat,
  annotations: &mut TextAnnotations,
) -> Range {
  annotations.clear_line_annotations();
  let pos = range.cursor(slice);
  let line_idx = slice.char_to_line(pos);
  let line_start = slice.line_to_char(line_idx);

  let orig_line_offset =
    slice.line(line_idx).len_chars().saturating_sub(line_start);

  let new_line_idx = match dir {
    Direction::Forward => line_idx.saturating_add(count),
    Direction::Backward => line_idx.saturating_sub(count),
  };

  let old_visual_position = translate_char_index_to_pos(slice, pos);

  let mut nls = 0;
  let new_pos = match slice.try_line_to_char(new_line_idx) {
    Ok(new_line_start) => {
      let new_line_length = slice.line(line_idx).len_chars();
      nls = new_line_start;
      new_line_start + orig_line_offset.min(new_line_length)
    },
    Err(e) => {
      log::error!("cursor out of bounds: {}", e);
      slice.len_chars() - 1
    },
  };

  log::warn!(
    "\ncount: {} move_vertically - oldpos: {}  newpos: {} old visual: {:?}\nnew_line_start: {} line_start: {}",
    count,
    pos,
    new_pos,
    old_visual_position,
    nls,
    line_start
  );
  // Special-case to avoid moving to the end of the last non-empty line.
  if slice.chars_at(new_line_idx).count() == 0 {
    return range;
  }

  let mut new_range =
    range.put_cursor(slice, new_pos, behaviour == Movement::Extend);

  new_range.old_visual_position =
    Some((old_visual_position.row as u32, old_visual_position.col as u32));

  new_range
}
