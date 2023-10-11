use std::{collections::HashMap, time::Duration};

use super::{Component, Frame};
use crate::{
  action::Action,
  components::session::Session,
  config::{Config, KeyBindings},
};
use async_openai::types::ChatCompletionResponseMessage;
use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use log::error;
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;
use tui_input::{backend::crossterm::EventHandler, Input};

#[derive(Default, Copy, Clone, PartialEq, Eq)]
pub enum Mode {
  #[default]
  Normal,
  Insert,
  Processing,
}
#[derive(Default)]
pub struct Home {
  pub show_help: bool,
  pub mode: Mode,
  pub input: Input,
  pub action_tx: Option<UnboundedSender<Action>>,
  pub keymap: HashMap<KeyEvent, Action>,
  pub text: Vec<String>,
  pub last_events: Vec<KeyEvent>,
  pub config: Config,
  pub session: Session,
}

impl Home {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn tick(&mut self) {
    log::info!("Tick");
    self.last_events.drain(..);
  }
}

impl Component for Home {
  fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
    self.action_tx = Some(tx);
    Ok(())
  }

  fn register_config_handler(&mut self, config: Config) -> Result<()> {
    self.config = config;
    Ok(())
  }

  fn update(&mut self, action: Action) -> Result<Option<Action>> {
    match action {
      Action::Tick => self.tick(),
      // Action::Render => self.render_tick(),
      // Action::ToggleShowHelp => self.show_help = !self.show_help,
      // Action::ScheduleIncrement => self.schedule_increment(1),
      // Action::ScheduleDecrement => self.schedule_decrement(1),
      // Action::Increment(i) => self.increment(i),
      // Action::Decrement(i) => self.decrement(i),
      // Action::ProcessResponse(s) => Action::Update(),
      Action::EnterNormal => {
        self.mode = Mode::Normal;
      },
      Action::EnterInsert => {
        self.mode = Mode::Insert;
      },
      Action::EnterProcessing => {
        self.mode = Mode::Processing;
      },
      Action::ExitProcessing => {
        // TODO: Make this go to previous mode instead
        self.mode = Mode::Normal;
      },
      _ => (),
    }
    Ok(None)
  }

  fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
    self.last_events.push(key.clone());
    let action = match self.mode {
      Mode::Normal | Mode::Processing => return Ok(None),
      Mode::Insert => match key.code {
        KeyCode::Esc => Action::EnterNormal,
        KeyCode::Enter => {
          if let Some(sender) = &self.action_tx {
            if let Err(e) = sender.send(Action::SubmitInput(self.input.value().to_string())) {
              error!("Failed to send action: {:?}", e);
            }
          }
          Action::EnterNormal
        },
        _ => {
          self.input.handle_event(&crossterm::event::Event::Key(key));
          Action::Update
        },
      },
    };
    Ok(Some(action))
  }

  fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
    let rects = Layout::default().constraints([Constraint::Percentage(100), Constraint::Min(3)].as_ref()).split(area);
    // let text: Vec<Line> = self.text.clone().iter().map(|l| Line::from(l.clone())).collect();
    let title_text = Line::from(vec![
      Span::raw("sazid semantic llvm console "),
      match self.mode {
        Mode::Normal => Span::styled("Normal Mode", Style::default().fg(Color::Green)),
        Mode::Insert => Span::styled("Insert Mode", Style::default().fg(Color::Yellow)),
        Mode::Processing => Span::styled("Processing", Style::default().fg(Color::Yellow)),
      },
    ]);
    f.render_widget(
      Block::default()
        .title(title_text)
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(match self.mode {
          Mode::Processing => Style::default().fg(Color::Yellow),
          _ => Style::default(),
        })
        .border_type(BorderType::Rounded),
      rects[0],
    );
    let width = rects[1].width.max(3) - 3; // keep 2 for borders and 1 for cursor
    let scroll = self.input.visual_scroll(width as usize);
    let input = Paragraph::new(self.input.value())
      .style(match self.mode {
        Mode::Insert => Style::default().fg(Color::Yellow),
        _ => Style::default(),
      })
      .scroll((0, scroll as u16))
      .block(Block::default().borders(Borders::ALL).title(Line::from(vec![
        Span::raw("Enter Input Mode "),
        Span::styled("(Press ", Style::default().fg(Color::DarkGray)),
        Span::styled("i", Style::default().add_modifier(Modifier::BOLD).fg(Color::Gray)),
        Span::styled(" to start, ", Style::default().fg(Color::DarkGray)),
        Span::styled("ESC", Style::default().add_modifier(Modifier::BOLD).fg(Color::Gray)),
        Span::styled(" to finish)", Style::default().fg(Color::DarkGray)),
      ])));
    f.render_widget(input, rects[1]);
    if self.mode == Mode::Insert {
      f.set_cursor((rects[1].x + 1 + self.input.cursor() as u16).min(rects[1].x + rects[1].width - 2), rects[1].y + 1)
    }
    Ok(())
  }
}
