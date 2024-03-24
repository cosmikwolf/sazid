use crate::{
  action::SessionAction,
  app::{
    database::data_manager::*, errors::SazidError,
    session_config::SessionConfig,
  },
};
use async_openai::types::ChatCompletionRequestMessage;
use core::result::Result;
use serde::{Deserialize, Serialize};
use tui::buffer::Buffer;

use crate::action::serialize_boxed_session_action;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DataManagerAction {
  CreateSession(SessionConfig),
  LoadSession(i64),
  AddMessageEmbedding(i64, i64, ChatCompletionRequestMessage),
  #[serde(serialize_with = "serialize_boxed_session_action")]
  SessionAction(Box<SessionAction>),
  Error(String),
}

impl DataManager {
  fn handle_action(
    &mut self,
    action: DataManagerAction,
  ) -> Result<Option<DataManagerAction>, SazidError> {
    let tx = self.action_tx.clone().unwrap();
    let model = self.model.clone();
    let db_url = self.db_url.clone();
    match action {
      DataManagerAction::CreateSession(config) => {
        tokio::spawn(async move {
          let session = add_session(&db_url, config).await.unwrap();
          tx.send(DataManagerAction::SessionAction(Box::new(
            SessionAction::CreateLoadSessionResponse(session),
          )))
          .unwrap()
        });
        Ok(None)
      },
      DataManagerAction::LoadSession(id) => {
        tokio::spawn(async move {
          let session = load_session(&db_url, id).await.unwrap();
          tx.send(DataManagerAction::SessionAction(Box::new(
            SessionAction::CreateLoadSessionResponse(session),
          )))
          .unwrap()
        });
        Ok(None)
      },
      DataManagerAction::AddMessageEmbedding(
        session_id,
        message_id,
        message,
      ) => {
        tokio::spawn(async move {
          match add_message_embedding(
            &db_url, session_id, message_id, model, message,
          )
          .await
          {
            Ok(id) => tx
              .send(DataManagerAction::SessionAction(Box::new(
                SessionAction::MessageEmbeddingSuccess(id),
              )))
              .unwrap(),
            Err(e) => tx
              .send(DataManagerAction::Error(format!(
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
  fn draw(&mut self, _b: &mut Buffer) -> Result<(), SazidError> {
    Ok(())
  }
}
