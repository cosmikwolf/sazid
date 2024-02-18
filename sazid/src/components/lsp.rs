use crate::app::lsp::helix_lsp_interface::LanguageServerInterface;

use super::Component;

use core::result::Result;
use crossterm::event::{KeyEvent, MouseEvent};
use helix_lsp::block_on;
use ratatui::layout::Rect;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
  action::Action,
  app::errors::SazidError,
  config::Config,
  tui::{Event, Frame},
};

impl Component for LanguageServerInterface {
  #[allow(unused_variables)]
  fn register_action_handler(
    &mut self,
    tx: UnboundedSender<Action>,
  ) -> Result<(), SazidError> {
    self.action_tx = Some(tx);
    Ok(())
  }
  #[allow(unused_variables)]
  fn register_config_handler(
    &mut self,
    config: Config,
  ) -> Result<(), SazidError> {
    Ok(())
  }
  fn init(&mut self, _area: Rect) -> Result<(), SazidError> {
    Ok(())
  }
  fn handle_events(
    &mut self,
    event: Option<Event>,
  ) -> Result<Option<Action>, SazidError> {
    let r = match event {
      Some(Event::Key(key_event)) => self.handle_key_events(key_event)?,
      Some(Event::Mouse(mouse_event)) => {
        self.handle_mouse_events(mouse_event)?
      },
      _ => None,
    };
    Ok(r)
  }
  #[allow(unused_variables)]
  fn handle_key_events(
    &mut self,
    key: KeyEvent,
  ) -> Result<Option<Action>, SazidError> {
    Ok(None)
  }
  #[allow(unused_variables)]
  fn handle_mouse_events(
    &mut self,
    mouse: MouseEvent,
  ) -> Result<Option<Action>, SazidError> {
    Ok(None)
  }
  #[allow(unused_variables)]
  fn update(&mut self, action: Action) -> Result<Option<Action>, SazidError> {
    Ok(match action {
      // Action::LspCheckServerNotifications => {
      //   futures::executor::block_on(self.check_server_notifications());
      //   None
      // },
      // Action::LspServerMessageReceived((id, call)) => {
      //   futures::executor::block_on(Self::handle_language_server_message(
      //     &mut self.lsp_progress,
      //     &mut self.language_servers,
      //     call,
      //     id,
      //     &mut self.status_msg,
      //     &mut self.workspaces,
      //   ));
      //   None
      // },
      _ => None,
    })
  }

  #[allow(unused_variables)]
  fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<(), SazidError> {
    Ok(())
  }
}
