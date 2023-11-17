use std::collections::HashMap;

use super::{Component, Frame};
use crate::{
  action::Action,
  app::{color_math::get_rainbow_and_inverse_colors, errors::SazidError},
  components::session::Session,
  config::Config,
  trace_dbg,
};

use color_eyre::{eyre::Result, owo_colors::OwoColorize};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use log::error;
use rand;
use ratatui::{prelude::*, widgets::*};

use tokio::sync::mpsc::UnboundedSender;
use tui_textarea::{CursorMove, TextArea};

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum Mode {
  #[default]
  Visual,
  Normal,
  Insert,
  Processing,
  Command,
}

#[derive(Debug, Default)]
pub struct Home<'a> {
  pub show_help: bool,
  pub status: Option<String>,
  pub mode: Mode,
  pub input: TextArea<'a>,
  pub action_tx: Option<UnboundedSender<Action>>,
  pub keymap: HashMap<KeyEvent, Action>,
  pub text: Vec<String>,
  pub last_events: Vec<KeyEvent>,
  pub config: Config,
  pub session: Session<'static>,
  pub control_pressed: bool,
  pub color_counter: u32,
  pub rgb: Color,
  pub inv_rgb: Color,
}

const MAX24BIT: u32 = 16777216;

impl<'a> Home<'a> {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn tick(&mut self) {
    //log::info!("Tick");
    self.last_events.drain(..);
  }

  pub fn replace_input(&mut self, text: String) {
    self.clear_input();
    self.input.move_cursor(CursorMove::Head);
    self.input.insert_str(&text);
  }

  pub fn clear_input(&mut self) {
    while !self.input.is_empty() {
      self.input.move_cursor(CursorMove::End);
      self.input.delete_line_by_head();
    }
  }
}

impl Component for Home<'static> {
  fn init(&mut self, _area: Rect) -> Result<(), SazidError> {
    self.color_counter = rand::random::<u32>() % MAX24BIT;
    self.input = TextArea::default();
    self.input.set_placeholder_text("press i to enter input mode");
    self.input.set_placeholder_style(Style::reset().fg(Color::Magenta));
    self.input.set_cursor_line_style(Style::reset().fg(Color::Yellow));

    self.input.set_cursor_style(Style::default().add_modifier(Modifier::SLOW_BLINK));
    Ok(())
  }
  fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<(), SazidError> {
    self.action_tx = Some(tx);
    Ok(())
  }

  fn register_config_handler(&mut self, config: Config) -> Result<(), SazidError> {
    self.config = config;
    Ok(())
  }

  fn update(&mut self, action: Action) -> Result<Option<Action>, SazidError> {
    match action {
      Action::Render => {
        self.color_counter += 100000;
        self.color_counter %= MAX24BIT;
        (self.rgb, self.inv_rgb) = get_rainbow_and_inverse_colors(self.color_counter, MAX24BIT);
        self.input.set_cursor_style(self.input.cursor_style().bg(self.rgb).fg(self.inv_rgb));
      },
      Action::Tick => self.tick(),
      // Action::Render => self.render_tick(),
      // Action::ToggleShowHelp => self.show_help = !self.show_help,
      // Action::ScheduleIncrement => self.schedule_increment(1),
      // Action::ScheduleDecrement => self.schedule_decrement(1),
      // Action::Increment(i) => self.increment(i),
      // Action::Decrement(i) => self.decrement(i),
      // Action::ProcessResponse(s) => Action::Update(),
      Action::UpdateStatus(s) => {
        trace_dbg!("update status: {:?}", s);
        self.status = s;
      },
      Action::EnterCommand => {
        self.mode = Mode::Command;
        self.session.mode = Mode::Command;
      },
      Action::EnterNormal => {
        self.mode = Mode::Normal;
        self.session.mode = Mode::Normal;
      },
      Action::EnterVisual => {
        self.mode = Mode::Visual;
        self.session.mode = Mode::Visual;
      },
      Action::EnterInsert => {
        trace_dbg!("enter insert mode");
        self.mode = Mode::Insert;
        self.session.mode = Mode::Insert;
      },
      Action::CommandResult(result) => {
        self.replace_input(result);
        self.mode = Mode::Command;
      },
      Action::EnterProcessing => {
        self.clear_input();
        self.mode = Mode::Processing;
        self.session.mode = Mode::Processing;
      },
      Action::ExitProcessing => {
        // TODO: Make this go to previous mode instead
        self.mode = Mode::Normal;
        self.session.mode = Mode::Normal;
      },
      _ => (),
    }
    Ok(None)
  }

  fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>, SazidError> {
    let tx = self.action_tx.clone().unwrap();
    self.last_events.push(key);

    let submit_input = |_a| {
      let input = self.input.lines().join("\n");
      // let string = format!("sending input: {}", input);
      // trace_dbg!(string);

      if let Err(e) = tx.send(Action::SubmitInput(input)) {
        error!("Failed to send action: {:?}", e);
      }
      Action::EnterNormal
    };

    let execute_command = |_c| {
      let input = self.input.lines().join("\n");
      if let Err(e) = tx.send(Action::ExecuteCommand(input)) {
        error!("Failed to send action: {:?}", e);
      }
      Action::EnterNormal
    };
    //trace_dbg!("key: {:#?}\n{:#?}", key, crossterm::event::Event::Key(key));
    trace_dbg!("insert key: {:?}\n{:?}", key, self.input.cursor());
    let action = match self.mode {
      Mode::Command => match key {
        KeyEvent { code: KeyCode::Esc, .. } => {
          self.clear_input();
          Action::EnterInsert
        },
        KeyEvent { code: KeyCode::Enter, .. } => {
          execute_command("");
          self.clear_input();
          Action::Update
        },
        _ => {
          self.input.input(crossterm::event::Event::Key(key));
          Action::Update
        },
      },
      Mode::Visual => match key {
        KeyEvent { code: KeyCode::Char(':'), .. } => {
          if self.input.cursor() == (0, 0) {
            Action::EnterCommand
          } else {
            Action::Update
          }
        },
        KeyEvent { code: KeyCode::Esc, .. } => Action::EnterNormal,
        KeyEvent { code: KeyCode::Enter, modifiers: KeyModifiers::ALT, .. } => submit_input(""),
        _ => Action::Update,
      },
      Mode::Normal | Mode::Processing => return Ok(None),
      Mode::Insert => match key {
        KeyEvent { code: KeyCode::Esc, .. } => Action::EnterVisual,
        KeyEvent { code: KeyCode::Enter, modifiers: KeyModifiers::ALT, .. } => submit_input(""),
        _ => {
          self.input.input(crossterm::event::Event::Key(key));
          Action::Update
        },
      },
    };
    Ok(Some(action))
  }

  fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<(), SazidError> {
    let input_length = self.input.clone().into_lines().len() as u16 + 2;
    let tx = self.action_tx.clone().unwrap();
    tx.send(Action::SetInputVsize(input_length)).unwrap();
    let rects =
      Layout::default().constraints([Constraint::Percentage(100), Constraint::Min(input_length)].as_ref()).split(area);
    // let text: Vec<Line> = self.text.clone().iter().map(|l| Line::from(l.clone())).collect();
    let title_text = Line::from(vec![
      Span::raw("sazid semantic llvm console "),
      match self.mode {
        Mode::Command => Span::styled("Command Mode", Style::default().fg(self.rgb)),
        Mode::Visual => Span::styled("Visual Mode", Style::default().fg(Color::Magenta)),
        Mode::Normal => Span::styled("Normal Mode", Style::default().fg(Color::Green)),
        Mode::Insert => Span::styled("Insert Mode", Style::default().fg(Color::Yellow)),
        Mode::Processing => Span::styled("Processing", Style::default().fg(self.rgb)),
      },
      match self.status {
        Some(ref s) => Span::styled(format!(": {}", s), Style::default().fg(Color::Yellow)),
        None => Span::raw(""),
      },
    ]);
    f.render_widget(
      Block::default()
        .title(title_text)
        .title_alignment(Alignment::Center)
        .borders(Borders::NONE)
        .border_style(match self.mode {
          Mode::Processing => Style::default().fg(Color::Yellow),
          _ => Style::default(),
        })
        .border_type(BorderType::Rounded),
      rects[0],
    );

    self.input.set_placeholder_text({
      match self.mode {
        Mode::Insert => "",
        _ => "press i to enter input mode",
      }
    });

    self.input.set_block(
      Block::default()
        .borders(Borders::ALL)
        .title(match self.mode {
          Mode::Command => Line::from(vec![
            Span::styled("Command Mode", Style::default().fg(self.rgb)),
            Span::styled("(press ", Style::default().fg(Color::DarkGray)),
            Span::styled("<alt>-<enter>", Style::default().add_modifier(Modifier::BOLD).fg(Color::Gray)),
            Span::styled(" to execute command, ", Style::default().fg(Color::DarkGray)),
            Span::styled("ESC", Style::default().add_modifier(Modifier::BOLD).fg(Color::Gray)),
            Span::styled(" to enter Insert mode)", Style::default().fg(Color::DarkGray)),
          ]),
          Mode::Insert => Line::from(vec![
            Span::raw("Input Mode"),
            Span::styled("(press", Style::default().fg(Color::DarkGray)),
            Span::styled("<alt>-<enter>", Style::default().add_modifier(Modifier::BOLD).fg(Color::Gray)),
            Span::styled(" to submit input, ", Style::default().fg(Color::DarkGray)),
            Span::styled("ESC", Style::default().add_modifier(Modifier::BOLD).fg(Color::Gray)),
            Span::styled(" to enter Visual mode)", Style::default().fg(Color::DarkGray)),
          ]),
          Mode::Visual => Line::from(vec![
            Span::raw("Visual Mode "),
            Span::styled("(Press ", Style::default().fg(Color::DarkGray)),
            Span::styled("i", Style::default().add_modifier(Modifier::BOLD).fg(Color::Gray)),
            Span::styled(" to enter text, ", Style::default().fg(Color::DarkGray)),
          ]),
          Mode::Processing => Line::from(vec![Span::raw("Awaiting Chat Completion")]),
          _ => Line::from(vec![
            Span::raw("Enter Input Mode "),
            Span::styled("(Press ", Style::default().fg(Color::DarkGray)),
            Span::styled("i", Style::default().add_modifier(Modifier::BOLD).fg(Color::Gray)),
            Span::styled(" to start, ", Style::default().fg(Color::DarkGray)),
            Span::styled("ESC", Style::default().add_modifier(Modifier::BOLD).fg(Color::Gray)),
            Span::styled(" to finish)", Style::default().fg(Color::DarkGray)),
          ]),
        })
        .style(match self.mode {
          Mode::Insert => Style::default().fg(Color::Yellow),
          Mode::Visual => Style::default().fg(Color::Cyan),
          Mode::Processing => Style::default().fg(self.rgb),
          _ => Style::default(),
        }),
    );
    f.render_widget(self.input.widget(), rects[1]);
    // let scroll = self.input.visual_scroll(width as usize);
    // let input = Paragraph::new(self.input.value())
    //   .style(match self.mode {
    //     Mode::Insert => Style::default().fg(Color::Yellow),
    //     _ => Style::default(),
    //   })
    //   .scroll((0, scroll as u16))
    //   .block(Block::default().borders(Borders::ALL).title(Line::from(vec![
    //     Span::raw("Enter n[]Input Mode "),
    //     Span::styled("(Press ", Style::default().fg(Color::DarkGray)),
    //     Span::styled("i", Style::default().add_modifier(Modifier::BOLD).fg(Color::Gray)),
    //     Span::styled(" to start, ", Style::default().fg(Color::DarkGray)),
    //     Span::styled("ESC", Style::default().add_modifier(Modifier::BOLD).fg(Color::Gray)),
    //     Span::styled(" to finish)", Style::default().fg(Color::DarkGray)),
    //   ])));
    //f.render_widget(input, rects[1]);
    if self.mode == Mode::Insert {
      //f.set_cursor((rects[1].x + 1).min(rects[1].x + rects[1].width - 2), rects[1].y + 1)
      //f.set_cursor((rects[1].x + 1 + self.input.cursor() as u16).min(rects[1].x + rects[1].width - 2), rects[1].y + 1)
    }
    Ok(())
  }
}
