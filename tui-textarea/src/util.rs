pub fn spaces(size: u8) -> &'static str {
    const SPACES: &str = "                                                                                                                                                                                                                                                                ";
    &SPACES[..size as usize]
}

pub fn num_digits(i: usize) -> u8 {
    f64::log10(i as f64) as u8 + 1
}

#[derive(Debug, Clone)]
pub struct Pos {
    pub row: usize,
    pub col: usize,
    pub offset: usize,
}

impl Pos {
    pub fn new(row: usize, col: usize, offset: usize) -> Self {
        Self { row, col, offset }
    }
}

#[cfg(feature = "ansi-escapes")]
pub fn plain_char_index_to_ansi_char_index(text: &str, index: usize) -> usize {
    let mut plain_index: usize = 0;
    let mut in_escape_sequence = false;
    let mut ansi_index: usize = 0;
    for c in text.chars() {
        if c == '\x1b' {
            in_escape_sequence = true;
        } else if in_escape_sequence {
            if c == 'm' {
                in_escape_sequence = false;
            }
        } else if c == '\n' || c == '\r' {
        } else {
            if plain_index == index {
                break;
            }
            plain_index += 1;
        }
        ansi_index += 1;
    }
    ansi_index.min(text.chars().count())
}

#[cfg(feature = "ansi-escapes")]
pub fn get_last_ansi_sequence(text: &str, index: Option<usize>) -> Option<String> {
    let mut ansi_sequences = Vec::new();
    let mut last_ansi_sequence = String::new();
    let mut in_escape_sequence = false;
    for c in text.chars() {
        if c == '\x1b' {
            last_ansi_sequence.push(c);
            in_escape_sequence = true;
        } else if in_escape_sequence {
            last_ansi_sequence.push(c);
            if c == 'm' {
                in_escape_sequence = false;
                if last_ansi_sequence != "\x1b[0m" {
                    ansi_sequences.push(last_ansi_sequence.clone());
                }
                last_ansi_sequence.clear();
            }
        }
        if let Some(index) = index {
            if ansi_sequences.len() == index {
                break;
            }
        }
    }
    ansi_sequences.last().cloned()
}
#[cfg(feature = "ansi-escapes")]
pub fn bookend_ansi_escapes(text: &[String]) -> Vec<String> {
    // find the last ansi formatting sequence in each line of text, and apply it to the beginning of the next line
    let mut bookended_text = Vec::new();
    let mut last_ansi_sequence = String::new();
    for line in text {
        let mut bookended_line = String::new();
        let mut in_escape_sequence = false;
        bookended_line.push_str(&last_ansi_sequence);
        for c in line.chars() {
            if c == '\u{1b}' {
                last_ansi_sequence.push(c);
                in_escape_sequence = true;
            } else if in_escape_sequence {
                last_ansi_sequence.push(c);
                if c == 'm' {
                    in_escape_sequence = false;
                }
            }
            bookended_line.push(c);
        }
        // add an ansi format reset sequence
        bookended_line.push_str("\x1b[0m");
        bookended_text.push(bookended_line);
    }
    bookended_text
}

#[cfg(feature = "ansi-escapes")]
pub fn as_plain_text(text: &[String]) -> Vec<String> {
    // strip all ansi escape sequences
    text.iter()
        .map(|line| {
            let mut plain_line = String::new();
            let mut in_escape_sequence = false;
            for c in line.chars() {
                if c == '\x1b' {
                    in_escape_sequence = true;
                } else if in_escape_sequence {
                    if c == 'm' {
                        in_escape_sequence = false;
                    }
                } else if c != '\n' || c != '\r' {
                    plain_line.push(c);
                }
            }
            plain_line
        })
        .collect::<Vec<_>>()
}

#[cfg(feature = "ansi-escapes")]
pub fn ansi_to_plain_text(text: &str) -> String {
    let mut plain_text = String::new();
    let mut in_escape_sequence = false;
    for c in text.chars() {
        if c == '\x1b' {
            in_escape_sequence = true;
        } else if in_escape_sequence {
            if c == 'm' {
                in_escape_sequence = false;
            }
        } else {
            plain_text.push(c);
        }
    }
    plain_text
}
