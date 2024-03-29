use crossterm::event::KeyEvent;
use ratatui::prelude::Rect;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

pub mod color_math;
pub mod consts;
pub mod database;
pub mod errors;
pub mod functions;
pub mod gpt_interface;
pub mod helpers;
pub mod lsp;
pub mod messages;
pub mod request_validation;
pub mod session_config;
pub mod session_view;
pub mod tools;
pub mod treesitter;
pub mod types;

use crate::{
  action::Action,
  components::{home::Home, session::Session, Component},
  config::Config,
  tui,
};

use self::{
  database::data_manager::{add_session, DataManager},
  errors::SazidError,
  session_config::SessionConfig,
};

#[derive(
  Default, Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize,
)]
pub enum Mode {
  #[default]
  Home,
}

pub struct App {
  pub config: Config,
  pub tick_rate: f64,
  pub frame_rate: f64,
  pub components: Vec<Box<dyn Component>>,
  pub should_quit: bool,
  pub should_suspend: bool,
  pub mode: Mode,
  pub last_tick_key_events: Vec<KeyEvent>,
}

pub fn add_session_sync(
  db_url: &str,
  config: SessionConfig,
) -> Result<database::types::QueryableSession, SazidError> {
  use diesel::prelude::{
    Connection, ExpressionMethods, RunQueryDsl, SelectableHelper,
  };
  let mut conn =
    diesel_async::async_connection_wrapper::AsyncConnectionWrapper::<
      diesel_async::AsyncPgConnection,
    >::establish(db_url)
    .unwrap();
  use crate::app::database::schema::sessions;
  let config = diesel_json::Json::new(config);
  let session = diesel::insert_into(sessions::table)
    // .values((dsl::model.eq(model.to_string()), dsl::rag.eq(rag)))
    .values(sessions::config.eq(&config))
    .returning(database::types::QueryableSession::as_returning())
    .get_result::<database::types::QueryableSession>(&mut conn)?;
  Ok(session)
}

impl App {
  pub async fn new(
    tick_rate: f64,
    frame_rate: f64,
    config: Config,
    data_manager: DataManager,
  ) -> Result<Self, SazidError> {
    let home = Home::new();
    let db_url = data_manager.get_database_url();
    let session: Session =
      add_session(&db_url, config.session_config.clone()).await.unwrap().into();
    let mode = Mode::Home;
    Ok(Self {
      tick_rate,
      frame_rate,
      components: vec![
        Box::new(home),
        Box::new(session),
        Box::new(data_manager),
      ],
      should_quit: false,
      should_suspend: false,
      config,
      mode,
      last_tick_key_events: Vec::new(),
    })
  }

  pub async fn run(&mut self) -> Result<(), SazidError> {
    let (action_tx, mut action_rx) = mpsc::unbounded_channel();

    let mut tui = tui::Tui::new().unwrap();
    tui.tick_rate(self.tick_rate);
    tui.frame_rate(self.frame_rate);
    tui.mouse(true);
    tui.enter().unwrap();

    for component in self.components.iter_mut() {
      component.register_action_handler(action_tx.clone()).unwrap();
    }

    for component in self.components.iter_mut() {
      component.register_config_handler(self.config.clone()).unwrap();
    }

    for component in self.components.iter_mut() {
      component.init(tui.size().unwrap()).unwrap();
    }

    loop {
      if let Some(e) = tui.next().await {
        match e {
          tui::Event::Quit => action_tx.send(Action::Quit).unwrap(),
          tui::Event::Tick => action_tx.send(Action::Tick).unwrap(),
          tui::Event::Render => action_tx.send(Action::Render).unwrap(),
          tui::Event::Resize(x, y) => {
            action_tx.send(Action::Resize(x, y)).unwrap()
          },
          tui::Event::Key(key) => {
            if let Some(keymap) = self.config.keybindings.get(&self.mode) {
              if let Some(action) = keymap.get(&vec![key]) {
                log::info!("Got action: {action:?}");
                action_tx.send(action.clone()).unwrap();
              } else {
                // If the key was not handled as a single key action,
                // then consider it for multi-key combinations.
                self.last_tick_key_events.push(key);

                // Check for multi-key combinations
                if let Some(action) = keymap.get(&self.last_tick_key_events) {
                  log::info!("Got action: {action:?}");
                  action_tx.send(action.clone()).unwrap();
                }
              }
            };
          },
          _ => {},
        }
        for component in self.components.iter_mut() {
          if let Some(action) =
            component.handle_events(Some(e.clone())).unwrap()
          {
            action_tx.send(action).unwrap();
          }
        }
      }

      while let Ok(action) = action_rx.try_recv() {
        if action != Action::Tick && action != Action::Render {
          //          log::debug!("{action:.unwrap()}");
        }
        match action {
          Action::Tick => {
            self.last_tick_key_events.drain(..);
          },
          Action::Quit => self.should_quit = true,
          Action::Suspend => self.should_suspend = true,
          Action::Resume => self.should_suspend = false,
          Action::Resize(w, h) => {
            //trace_dbg!("Action::Resize");
            tui.resize(Rect::new(0, 0, w, h)).unwrap();
            tui
              .draw(|f| {
                for component in self.components.iter_mut() {
                  let r = component.draw(f, f.size());
                  if let Err(e) = r {
                    action_tx
                      .send(Action::Error(format!("Failed to draw: {:?}", e)))
                      .unwrap();
                  }
                }
              })
              .unwrap();
          },
          Action::Render => {
            //trace_dbg!("Action::Render");
            tui
              .draw(|f| {
                for component in self.components.iter_mut() {
                  let r = component.draw(f, f.size());
                  if let Err(e) = r {
                    action_tx
                      .send(Action::Error(format!("Failed to draw: {:?}", e)))
                      .unwrap();
                  }
                }
              })
              .unwrap();
          },
          _ => {},
        }
        for component in self.components.iter_mut() {
          if let Some(action) = component.update(action.clone()).unwrap() {
            action_tx.send(action).unwrap()
          };
        }
      }
      if self.should_suspend {
        tui.suspend().unwrap();
        action_tx.send(Action::Resume).unwrap();
        tui = tui::Tui::new().unwrap();
        tui.tick_rate(self.tick_rate);
        tui.frame_rate(self.frame_rate);
        tui.mouse(true);
        tui.enter().unwrap();
      } else if self.should_quit {
        tui.stop().unwrap();
        break;
      }
    }
    tui.exit().unwrap();
    Ok(())
  }
}
