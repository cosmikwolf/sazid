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

    let mut current_line_width = self.current_line.chars().map(|c| c.len_utf16() as u16).sum();
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
        self.words.next();
        break;
      }

      let word_width = word.chars().map(|c| c.len_utf16() as u16).sum::<u16>();

      if current_line_width + word_width > self.max_line_width {
        // If the current word doesn't fit, break the line.
        break;
      }

      self.current_line.push_str(word);
      current_line_width += word_width;

      self.words.next();
    }

    // Trim trailing whitespace from the current line.
    self.current_line = self.current_line.trim_end().to_string();
    current_line_width = self.current_line.chars().map(|c| c.len_utf16() as u16).sum();

    // Push the remainder to the next line, removing leading spaces.
    while let Some(&word) = self.words.peek() {
      if word.chars().all(|c| c.is_whitespace()) {
        self.words.next();
      } else {
        break;
      }
    }

    // Even if the iterator is exhausted, pass the previous remainder.
    if words_exhausted && self.current_line.is_empty() {
      None
    } else {
      Some((&self.current_line, current_line_width))
    }
  }
}
