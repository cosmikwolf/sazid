use helix_core::{
  doc_formatter::TextFormat,
  movement::{Direction, Movement},
  text_annotations::TextAnnotations,
  Position, Range, RopeSlice,
};
use helix_view::graphics::Rect;

pub fn translate_pos_to_char_index(
  text: &RopeSlice<'_>,
  area: Rect,
  vertical_scroll: u16,
  pos: helix_core::Position,
) -> Option<usize> {
  if pos.row < area.top() as usize
    || pos.row >= area.bottom() as usize
    || pos.col < area.left() as usize
    || pos.col >= area.right() as usize
  {
    None
  } else {
    let row = pos.row.saturating_sub(area.top() as usize) + vertical_scroll as usize;
    let col = pos.col - area.left() as usize;
    if let Ok(row_start_index) = text.try_line_to_char(row) {
      let line_max = text.line(row).len_chars();
      let index = row_start_index + col.min(line_max.saturating_sub(1));
      Some(index.min(text.len_chars().saturating_sub(1)))
    } else {
      Some(text.len_chars().saturating_sub(1))
    }
  }
}

pub fn translate_char_index_to_abs_pos(text: &RopeSlice<'_>, index: usize) -> Position {
  let row = text.char_to_line(index);
  let row_start_index = text.line_to_char(row);
  let col = index - row_start_index;
  Position::new(row, col)
}

pub fn translate_char_index_to_viewport_pos(
  text: &RopeSlice<'_>,
  area: Rect,
  vertical_scroll: u16,
  index: usize,
  debug: bool,
) -> (u16, Option<Direction>, helix_core::Position) {
  // log::info!("translate_char_index_to_pos: index: {}", index);
  let index = index.min(text.len_chars().saturating_sub(1));
  let row = text.char_to_line(index);
  let row_start_index = text.line_to_char(row);
  let col = index - row_start_index;

  let top_visible_line_idx = vertical_scroll as usize;
  let bottom_visible_line_idx = top_visible_line_idx + area.height.saturating_sub(1) as usize;

  let (scroll, direction, row) = {
    if row < top_visible_line_idx {
      let scroll = top_visible_line_idx - row;
      (scroll as u16, Some(Direction::Backward), row + scroll)
    } else if row > bottom_visible_line_idx {
      let scroll = row - bottom_visible_line_idx;
      (scroll as u16, Some(Direction::Forward), row.saturating_sub(scroll))
    } else {
      (0, None, row)
    }
  };

  let row = row.saturating_sub(vertical_scroll as usize) + area.top() as usize;
  let col = col + area.left() as usize;
  if debug {
    log::warn!(
      "area: {:?}  top_visible_line_idx: {}, bottom_visible_line_idx: {}
 index: {} raw_row: {} vertical_scroll: {}
row: {} col: {}
scroll: {}, direction: {:?}",
      area,
      top_visible_line_idx,
      bottom_visible_line_idx,
      index,
      text.char_to_line(index),
      vertical_scroll,
      row,
      col,
      scroll,
      direction,
    );
  }
  // let (scroll, direction, row) = {
  //   if vertical_scroll > row as u16 {
  //     (vertical_scroll - row as u16, Some(Direction::Backward), row + area.top() as usize)
  //   } else if row as u16 - vertical_scroll > area.height {
  //     let row = row.saturating_sub(vertical_scroll as usize);
  //     (
  //       row.saturating_sub(area.height as usize) as u16,
  //       Some(Direction::Forward),
  //       row + area.top() as usize,
  //     )
  //   } else {
  //     (0, None, row + area.top() as usize)
  //   }
  // };

  (scroll, direction, helix_core::Position::new(row, col))
}

pub fn put_cursor(range: Range, text: RopeSlice, char_idx: usize, extend: bool) -> Range {
  log::info!(
    "extend:{}\n text_len: {} char_idx: {}   head: {}  anchor: {} ",
    extend,
    text.len_chars(),
    char_idx,
    range.head,
    range.anchor
  );
  let char_idx = char_idx.clamp(0, text.len_chars() - 1);
  if extend {
    Range::new(range.anchor, char_idx)
    // if range.head == char_idx {
    //   log::info!(
    //     "put_cursor:  anchor <= char_idx\n char_idx: {}   head: {}  anchor: {} ",
    //     char_idx,
    //     range.head,
    //     range.anchor
    //   );
    //   Range::new(char_idx, range.head)
    // } else {
    //   log::info!(
    //     "put_cursor: anchor > char_idx\n char_idx: {}   head: {}  anchor: {} ",
    //     char_idx,
    //     range.head,
    //     range.anchor
    //   );
    //   Range::new(range.anchor, char_idx)
    // }
  } else {
    Range::point(char_idx)
  }
}

pub fn min_width_1(range: &helix_core::selection::Range) -> helix_core::selection::Range {
  if range.anchor == range.head {
    helix_core::selection::Range {
      anchor: range.head,
      head: range.anchor + 1,
      old_visual_position: range.old_visual_position,
    }
  } else {
    *range
  }
}
#[allow(clippy::too_many_arguments)]
pub fn session_move_horizontally(
  all_messages_text: RopeSlice,
  range: Range,
  dir: Direction,
  count: usize,
  behaviour: Movement,
  _: &TextFormat,
  _: &mut TextAnnotations,
) -> Range {
  let pos = range.head;
  let original_row = all_messages_text.char_to_line(pos);
  let original_row_start = all_messages_text.line_to_char(original_row);
  let original_row_len = all_messages_text.line(original_row).len_chars();
  let _original_row_end = original_row_start + original_row_len.saturating_sub(1);
  // Compute the new position.
  let new_pos = match dir {
    Direction::Forward => pos + count,
    Direction::Backward => pos.saturating_sub(count),
  }
  .clamp(0, all_messages_text.len_chars() - 1);

  log::warn!("move_horizontally original_pos: {}, new_pos: {}", pos, new_pos);
  // Compute the final new range.
  put_cursor(range, all_messages_text, new_pos, behaviour == Movement::Extend)
}

#[allow(clippy::too_many_arguments)]
pub fn session_move_vertically(
  all_messages_text: RopeSlice,
  range: Range,
  dir: Direction,
  count: usize,
  behaviour: Movement,
  _: &TextFormat,
  _: &mut TextAnnotations,
) -> Range {
  // annotations.clear_line_annotations();
  // log::info!("session_move_vertically\ndir: {:?} count: {}", dir, count);
  let pos = range.head;

  let original_row = all_messages_text.char_to_line(pos);
  let original_row_start = all_messages_text.line_to_char(original_row);
  let original_col = pos - original_row_start;
  let new_row = match dir {
    Direction::Forward => original_row + count,
    Direction::Backward => original_row.saturating_sub(count),
  };

  let new_row_length = match all_messages_text.get_line(new_row) {
    Some(row) => row.len_chars(),
    None => {
      log::error!("can't get row, reached end or begnning of messages");
      match dir {
        Direction::Forward => {
          return put_cursor(
            range,
            all_messages_text,
            all_messages_text.len_chars().saturating_sub(1),
            behaviour == Movement::Extend,
          )
        },
        Direction::Backward => {
          return put_cursor(range, all_messages_text, 0, behaviour == Movement::Extend)
        },
      }
    },
  };
  let new_col = original_col.min(new_row_length);
  let new_row_start = all_messages_text.line_to_char(new_row);
  let new_pos = new_row_start + new_col;

  log::warn!("count: {} move_vertically original_pos: {}, new_pos: {}", count, pos, new_pos,);

  put_cursor(range, all_messages_text, new_pos, behaviour == Movement::Extend)
  /*
  let mut msg_start_pos = 0;
  let (original_pos_message_index, original_pos_message) =
    match messages.iter().enumerate().find(|(i, msg)| {
      let msg_len = msg.plain_text.len_chars();
      if msg_start_pos + msg_len >= pos {
        true
      } else {
        msg_start_pos += msg_len + row_separator as usize;
        false
      }
    }) {
      Some((i, message)) => (i, message),
      None => return range,
    };

  let original_row =
    match original_pos_message.plain_text.try_char_to_line(pos - msg_start_pos)
    {
      Ok(original_col) => original_col,
      Err(_e) => {
        // handling cursor lands on row separator condition, by moving to start of next row
        log::error!(
          "returning 0 for line_idx, should be because we are betweeen lines"
        );
        0
      },
    };

  let original_row_start =
    original_pos_message.plain_text.line_to_char(original_row);
  let original_col = pos - original_row_start;

  // if new_row does not exist in this message, find the message where it does exist
  let (new_message, new_row) = match dir {
    Direction::Forward => {
      let lines_left_in_original =
        original_pos_message.plain_text.len_lines() - original_row - 1;
      let mut lines_to_go = count as isize;
      let mut acc_rows = 0;
      lines_to_go -= lines_left_in_original as isize;
      if lines_to_go <= 0 {
        log::info!("scrolling within message");
        (original_pos_message, original_row + count)
      } else {
        match messages.iter().skip(original_pos_message_index + 1).find(|msg| {
          lines_to_go -= msg.plain_text.len_lines() as isize;
          lines_to_go <= 0
        }) {
          Some(message) => {
            log::info!("scrolling to next message");
            (message, (-lines_to_go) as usize)
          },
          None => {
            log::warn!("scroll has gone beyond end");
            return range.put_cursor(
              all_messages_text,
              all_messages_text.len_chars() - 1,
              behaviour == Movement::Extend,
            );
          },
        }
      }
    },
    Direction::Backward => {
      let mut lines_to_go = count as isize;
      lines_to_go -= original_row as isize;
      if lines_to_go <= 0 {
        (original_pos_message, original_row - count)
      } else {
        match messages.iter().take(original_pos_message_index + 1).find(|msg| {
          lines_to_go -= msg.plain_text.len_lines() as isize;
          lines_to_go <= 0
        }) {
          Some(message) => (
            message,
            (message.plain_text.len_lines() as isize + lines_to_go) as usize,
          ),
          None => {
            log::warn!("scroll has gone beyond beginning");
            return range.put_cursor(
              all_messages_text,
              0,
              behaviour == Movement::Extend,
            );
          },
        }
      }
    },
  };

  log::info!(
    "original_row: {}, new_row: {}\n{:?}",
    original_row,
    new_row,
    new_message.plain_text
  );

  let new_row_len = match new_message.plain_text.get_line(new_row) {
    Some(row) => row.len_chars(),
    None => {
      panic!("can't get new row");
    },
  };

  let new_col = original_col.min(new_row_len);

  let new_row_start = new_message.plain_text.line_to_char(new_row);
  let new_pos = new_row_start + new_col;

  log::warn!(
    "count: {} move_vertically original_pos: {}, new_pos: {}",
    count,
    pos,
    new_pos,
  );

  range.put_cursor(all_messages_text, new_pos, behaviour == Movement::Extend)
      */
}
