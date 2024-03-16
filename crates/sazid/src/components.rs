use core::result::Result;
// This is a sample comment
use helix_view::graphics::Rect;
use tokio::sync::mpsc::UnboundedSender;

use crate::{action::Action, app::errors::SazidError, config::Config};

use tui::buffer::Buffer;

pub mod data_manager;
pub mod session;

pub trait Component {
  #[allow(unused_variables)]
  fn register_action_handler(
    &mut self,
    tx: UnboundedSender<Action>,
  ) -> Result<(), SazidError> {
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
  #[allow(unused_variables)]
  fn update(&mut self, action: Action) -> Result<Option<Action>, SazidError> {
    Ok(None)
  }
  fn draw(&mut self, b: &mut Buffer) -> Result<(), SazidError>;
}
