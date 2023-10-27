use core::result::Result;
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::layout::Rect;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
  action::Action,
  app::errors::SazidError,
  config::Config,
  tui::{Event, Frame},
};

pub mod home;
pub mod session;

pub trait Component {
  #[allow(unused_variables)]
  fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<(), SazidError> {
    Ok(())
  }
  #[allow(unused_variables)]
  fn register_config_handler(&mut self, config: Config) -> Result<(), SazidError> {
    Ok(())
  }
  fn init(&mut self, _area: Rect) -> Result<(), SazidError> {
    Ok(())
  }
  fn handle_events(&mut self, event: Option<Event>) -> Result<Option<Action>, SazidError> {
    let r = match event {
      Some(Event::Key(key_event)) => self.handle_key_events(key_event)?,
      Some(Event::Mouse(mouse_event)) => self.handle_mouse_events(mouse_event)?,
      _ => None,
    };
    Ok(r)
  }
  #[allow(unused_variables)]
  fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>, SazidError> {
    Ok(None)
  }
  #[allow(unused_variables)]
  fn handle_mouse_events(&mut self, mouse: MouseEvent) -> Result<Option<Action>, SazidError> {
    Ok(None)
  }
  #[allow(unused_variables)]
  fn update(&mut self, action: Action) -> Result<Option<Action>, SazidError> {
    Ok(None)
  }
  fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<(), SazidError>;
}
