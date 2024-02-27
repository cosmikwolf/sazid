use crate::{
  action::Action,
  app::{database::data_manager::*, errors::SazidError},
  compositor::{Component, Context},
  config::Config,
  sazid_tui::Event,
  trace_dbg,
};
use core::result::Result;
use helix_view::graphics::Rect;
use tokio::sync::mpsc::UnboundedSender;
use tui::buffer::Buffer;

use tui::buffer::Buffer as Surface;

impl Component for DataManager {
  fn render(&mut self, area: Rect, frame: &mut Surface, ctx: &mut Context) {}
}

impl DataManager {
  fn register_action_handler(
    &mut self,
    tx: UnboundedSender<Action>,
  ) -> Result<(), SazidError> {
    trace_dbg!("register_session_action_handler");
    self.action_tx = Some(tx);
    Ok(())
  }

  fn register_config_handler(
    &mut self,
    _config: Config,
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
    // let r = match event {
    //   Some(Event::Key(key_event)) => self.handle_key_events(key_event)?,
    //   Some(Event::Mouse(mouse_event)) => {
    //     self.handle_mouse_events(mouse_event)?
    //   },
    //   _ => None,
    // };
    // Ok(r)
    Ok(None)
  }

  fn update(&mut self, action: Action) -> Result<Option<Action>, SazidError> {
    let tx = self.action_tx.clone().unwrap();
    let db_url = self.get_database_url();
    let model = self.model.clone();
    match action {
      Action::CreateSession(config) => {
        tokio::spawn(async move {
          let session = add_session(&db_url, config).await.unwrap();
          tx.send(Action::CreateLoadSessionResponse(session)).unwrap()
        });
        Ok(None)
      },
      Action::LoadSession(id) => {
        tokio::spawn(async move {
          let session = load_session(&db_url, id).await.unwrap();
          tx.send(Action::CreateLoadSessionResponse(session)).unwrap()
        });
        Ok(None)
      },
      Action::AddMessageEmbedding(session_id, message_id, message) => {
        tokio::spawn(async move {
          match add_message_embedding(
            &db_url, session_id, message_id, model, message,
          )
          .await
          {
            Ok(id) => tx.send(Action::MessageEmbeddingSuccess(id)).unwrap(),
            Err(e) => tx
              .send(Action::Error(format!(
                "embeddings_manager- update: {:#?}",
                e
              )))
              .unwrap(),
          }
        });
        Ok(None)
      },
      _ => Ok(None),
    }
  }
  fn draw(&mut self, b: &mut Buffer) -> Result<(), SazidError> {
    Ok(())
  }
}
