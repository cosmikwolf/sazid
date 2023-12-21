use crate::{
  action::Action,
  app::{
    database::embeddings_manager::{add_message_embedding, add_session, load_session, EmbeddingsManager},
    errors::SazidError,
  },
  config::Config,
  trace_dbg,
  tui::{Event, Frame},
};
use async_openai::config::OpenAIConfig;
use core::result::Result;
use diesel::prelude::*;
use futures_util::TryFutureExt;
use ratatui::prelude::Rect;
use tokio::sync::mpsc::UnboundedSender;

use super::Component;

impl Component for EmbeddingsManager {
  fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<(), SazidError> {
    trace_dbg!("register_session_action_handler");
    self.action_tx = Some(tx);
    Ok(())
  }

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
  fn update(&mut self, action: Action) -> Result<Option<Action>, SazidError> {
    let tx = self.action_tx.clone().unwrap();
    let db_url = self.get_database_url();
    let model = self.model.clone();
    match action {
      Action::CreateSession(model, prompt, rag) => {
        tokio::spawn(async move {
          let id = add_session(&db_url, &model, rag).await.unwrap();
          tx.send(Action::CreateSessionResponse(id)).unwrap()
        });
        Ok(None)
      },
      Action::LoadSession(id) => {
        tokio::spawn(async move {
          let session = load_session(&db_url, id).await.unwrap();
          tx.send(Action::CreateSessionResponse(id)).unwrap()
        });
        Ok(None)
      },
      Action::AddMessageEmbedding(id, message_container) => {
        if message_container.receive_complete {
          tokio::spawn(async move {
            add_message_embedding(&db_url, id, message_container.stream_id, model, message_container.message)
              .await
              .unwrap()
          });
        }
        Ok(None)
      },
      _ => Ok(None),
    }
  }
  fn draw(&mut self, _f: &mut Frame<'_>, area: Rect) -> Result<(), SazidError> {
    Ok(())
  }
}
