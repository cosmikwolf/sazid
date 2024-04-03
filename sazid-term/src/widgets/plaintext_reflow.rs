pub trait LineComposerStr<'a> {
  fn next_line(&mut self) -> Option<(&str, u16)>;
}

pub struct WordWrapperStr<'a> {
  words: std::iter::Peekable<Box<dyn Iterator<Item = &'a str> + 'a>>,
  max_line_width: u16,
  current_line: String,
  next_line: String,
  trim: bool,
}

impl<'a> WordWrapperStr<'a> {
  pub fn new(
    words: Box<dyn Iterator<Item = &'a str> + 'a>,
    max_line_width: u16,
    trim: bool,
  ) -> WordWrapperStr<'a> {
    WordWrapperStr {
      words: words.peekable(),
      max_line_width,
      current_line: String::new(),
      next_line: String::new(),
      trim,
    }
  }
}

impl<'a> LineComposerStr<'a> for WordWrapperStr<'a> {
  fn next_line(&mut self) -> Option<(&str, u16)> {
    if self.max_line_width == 0 {
      return None;
    }
    std::mem::swap(&mut self.current_line, &mut self.next_line);
    self.next_line.clear();
    let mut current_line_width =
      self.current_line.chars().map(|c| c.len_utf16() as u16).sum();
    let mut width_to_last_word_end: u16 = 0;
    let mut prev_whitespace = false;
    let mut words_exhausted = true;
    while let Some(&word) = self.words.peek() {
      words_exhausted = false;
      let word_whitespace = word.chars().all(|c| c.is_whitespace());
      // Ignore words wider than the total max width.
      if word.chars().map(|c| c.len_utf16() as u16).sum::<u16>() > self.max_line_width
                // Skip leading whitespace when trim is enabled.
                || self.trim && word_whitespace && current_line_width == 0
      {
        self.words.next();
        continue;
      }
      // Break on newline and discard it.
      if word == "\n" || word == "\r\n" {
        if prev_whitespace {
          current_line_width = width_to_last_word_end;
          self.current_line.truncate(self.current_line.trim_end().len());
        }
        self.words.next();
        break;
      }
      // Mark the previous word as word end.
      if word_whitespace && !prev_whitespace {
        width_to_last_word_end = current_line_width;
      }
      self.current_line.push_str(word);
      current_line_width +=
        word.chars().map(|c| c.len_utf16() as u16).sum::<u16>();
      if current_line_width > self.max_line_width {
        // If there was no word break in the text, wrap at the end of the line.
        let (truncate_at, truncated_width) = if width_to_last_word_end != 0 {
          (self.current_line.trim_end().len(), width_to_last_word_end)
        } else {
          (self.current_line.len(), self.max_line_width)
        };
        // Push the remainder to the next line but strip leading whitespace:
        let remainder = &self.current_line[truncate_at..];
        if let Some(remainder_nonwhite) =
          remainder.find(|c: char| !c.is_whitespace())
        {
          self.next_line.push_str(&remainder[remainder_nonwhite..]);
        }
        self.current_line.truncate(truncate_at);
        current_line_width = truncated_width;
        break;
      }
      prev_whitespace = word_whitespace;
      self.words.next();
    }
    // Even if the iterator is exhausted, pass the previous remainder.
    if words_exhausted && self.current_line.is_empty() {
      None
    } else {
      Some((&self.current_line, current_line_width))
    }
  }
}
