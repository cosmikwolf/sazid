use crossterm;
use crossterm::event::KeyEvent;
use crossterm::event::{KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
use crossterm::execute;
use std::io::{stdout, Write};

fn main() {
  println!("press some ctrl-<key> combos. press ctrl-c to exit");
  execute!(
    stdout(),
    PushKeyboardEnhancementFlags(
      KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
        | KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
        | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
    )
  )
  .unwrap();
  while true {
    stdout().flush().unwrap();
    match crossterm::event::read().unwrap() {
      crossterm::event::Event::Key(key_event) => {
        println!("{:?}", key_event);
      },
      _ => {
        println!("Not read as key event.")
      },
    }
  }
  execute!(stdout(), PopKeyboardEnhancementFlags).unwrap();
}

// OUTPUT
// Press Ctrl+[ : KeyEvent { code: Esc, modifiers: NONE }
// Press Ctrl+] : KeyEvent { code: Char('5'), modifiers: CONTROL }
// Press Ctrl+/ : KeyEvent { code: Char('7'), modifiers: SHIFT | CONTROL }
// Press Ctrl+\ : KeyEvent { code: Char('4'), modifiers: CONTROL }
