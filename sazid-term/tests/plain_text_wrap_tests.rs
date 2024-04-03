#[cfg(test)]
mod tests {
  use sazid_term::widgets::plaintext_reflow::LineComposerStr;
  use sazid_term::widgets::plaintext_reflow::WordWrapperStr;
  use unicode_segmentation::UnicodeSegmentation;

  #[test]
  fn test_empty_string() {
    let input = "";
    let words = Box::new(input.unicode_words());
    let mut wrapper = WordWrapperStr::new(words, 10, true);
    assert_eq!(wrapper.next_line(), None);
  }

  #[test]
  fn test_single_line() {
    let input = "Hello, world!";
    let words = Box::new(input.unicode_words());
    let mut wrapper = WordWrapperStr::new(words, 20, true);
    assert_eq!(wrapper.next_line(), Some(("Hello, world!", 13)));
    assert_eq!(wrapper.next_line(), None);
  }

  #[test]
  fn test_multiple_lines() {
    let input = "System\nyou are an expert programming assistant";
    let words = Box::new(input.unicode_words());
    let mut wrapper = WordWrapperStr::new(words, 20, true);
    assert_eq!(wrapper.next_line(), Some(("System", 6)));
    assert_eq!(wrapper.next_line(), Some(("you are an expert", 17)));
    assert_eq!(wrapper.next_line(), Some(("programming", 11)));
    assert_eq!(wrapper.next_line(), Some(("assistant", 9)));
    assert_eq!(wrapper.next_line(), None);
  }

  #[test]
  fn test_long_word() {
    let input = "A veryverylongword that exceeds the max width.";
    let words = Box::new(input.unicode_words());
    let mut wrapper = WordWrapperStr::new(words, 15, true);
    assert_eq!(wrapper.next_line(), Some(("A", 1)));
    assert_eq!(wrapper.next_line(), Some(("veryverylongword", 16)));
    assert_eq!(wrapper.next_line(), Some(("that exceeds", 12)));
    assert_eq!(wrapper.next_line(), Some(("the max width.", 14)));
    assert_eq!(wrapper.next_line(), None);
  }

  #[test]
  fn test_leading_whitespace() {
    let input = "  Leading whitespace.";
    let words = Box::new(input.unicode_words());
    let mut wrapper = WordWrapperStr::new(words, 20, true);
    assert_eq!(wrapper.next_line(), Some(("Leading whitespace.", 19)));
    assert_eq!(wrapper.next_line(), None);
  }

  #[test]
  fn test_trailing_whitespace() {
    let input = "Trailing whitespace.   ";
    let words = Box::new(input.unicode_words());
    let mut wrapper = WordWrapperStr::new(words, 20, true);
    assert_eq!(wrapper.next_line(), Some(("Trailing whitespace.", 20)));
    assert_eq!(wrapper.next_line(), None);
  }
}
