use arc_swap::{access::Map, ArcSwap};
use futures_util::Stream;
use helix_core::{diagnostic::Severity, pos_at_coords, syntax, Selection};
use helix_lsp::{
  lsp::{self, notification::Notification},
  util::lsp_range_to_range,
  LspProgressMap,
};
use helix_stdx::path::get_relative_path;
use helix_view::{
  align_view,
  document::DocumentSavedEventResult,
  editor::{ConfigEvent, EditorEvent},
  graphics::Rect,
  theme,
  tree::Layout,
  Align, Editor,
};
use serde_json::json;
use tui::backend::Backend;

// use helix_term::{
//   args::Args,
//   config::Config,
//   keymap::Keymaps,
//   ui::{self, overlay::overlaid},
// };

use crate::compositor::{Component, Compositor, Event};
use crate::job::Jobs;

use log::{debug, error, info, warn};
#[cfg(not(feature = "integration"))]
use std::io::stdout;
use std::{collections::btree_map::Entry, io::stdin, path::Path, sync::Arc};

use anyhow::{Context, Error};

use crossterm::{event::Event as CrosstermEvent, tty::IsTty};
#[cfg(not(windows))]
use {signal_hook::consts::signal, signal_hook_tokio::Signals};
#[cfg(windows)]
type Signals = futures_util::stream::Empty<()>;

#[cfg(not(feature = "integration"))]
use tui::backend::CrosstermBackend;

#[cfg(feature = "integration")]
use tui::backend::TestBackend;

#[cfg(not(feature = "integration"))]
type TerminalBackend = CrosstermBackend<std::io::Stdout>;

#[cfg(feature = "integration")]
type TerminalBackend = TestBackend;

type Terminal = tui::terminal::Terminal<TerminalBackend>;

use crossterm::event::KeyEvent;
use helix_view::graphics::CursorKind;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

pub mod color_math;
pub mod consts;
pub mod database;
pub mod errors;
pub mod functions;
pub mod gpt_interface;
pub mod helpers;
pub mod lsp_interface;
pub mod markdown;
pub mod messages;
pub mod request_validation;
pub mod session_config;
pub mod session_view;
pub mod tools;
pub mod treesitter;
pub mod types;

use crate::{
  action::Action,
  components::{home::Home, session::Session},
  config::Config,
  sazid_tui,
};
use tui;

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
  compositor: Compositor,
  terminal: Terminal,

  config: Arc<ArcSwap<Config>>,

  #[allow(dead_code)]
  theme_loader: Arc<theme::Loader>,
  #[allow(dead_code)]
  syn_loader: Arc<syntax::Loader>,

  signals: Signals,
  jobs: Jobs,
  lsp_progress: LspProgressMap,
  pub cursor: CursorKind,
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
    data_manager: DataManager,
    config: Config,
    syn_loader_conf: syntax::Configuration,
  ) -> Result<Self, SazidError> {
    let home = Home::new();
    let db_url = data_manager.get_database_url();
    let session: Session =
      add_session(&db_url, config.session_config.clone()).await.unwrap().into();
    let mode = Mode::Home;

    let mut theme_parent_dirs = vec![helix_loader::config_dir()];
    theme_parent_dirs.extend(helix_loader::runtime_dirs().iter().cloned());
    let theme_loader =
      std::sync::Arc::new(theme::Loader::new(&theme_parent_dirs));

    let syn_loader = std::sync::Arc::new(syntax::Loader::new(syn_loader_conf));

    #[cfg(not(feature = "integration"))]
    let backend = CrosstermBackend::new(stdout(), &config.editor);

    #[cfg(feature = "integration")]
    let backend = TestBackend::new(120, 150);

    let terminal = Terminal::new(backend)?;
    let area = terminal.size().expect("couldn't get terminal size");
    let mut compositor = Compositor::new(area);
    let config = Arc::new(ArcSwap::from_pointee(config));

    #[cfg(windows)]
    let signals = futures_util::stream::empty();
    #[cfg(not(windows))]
    let signals = Signals::new([
      signal::SIGTSTP,
      signal::SIGCONT,
      signal::SIGUSR1,
      signal::SIGTERM,
      signal::SIGINT,
    ])
    .context("build signal handler")
    .unwrap();

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
      cursor: CursorKind::Block,
      mode,
      last_tick_key_events: Vec::new(),

      compositor,
      terminal,

      config,

      theme_loader,
      syn_loader,

      signals,
      jobs: Jobs::new(),
      lsp_progress: LspProgressMap::new(),
    })
  }

  pub async fn run(&mut self) -> Result<(), SazidError> {
    let (action_tx, mut action_rx) = mpsc::unbounded_channel();

    let mut tui = sazid_tui::Tui::new().unwrap();
    tui.tick_rate(self.tick_rate);
    tui.frame_rate(self.frame_rate);
    tui.mouse(true);
    tui.enter().unwrap();

    for component in self.components.iter_mut() {
      component.register_action_handler(action_tx.clone()).unwrap();
    }

    for component in self.components.iter_mut() {
      component.register_config_handler(self.sazid_config.clone()).unwrap();
    }

    for component in self.components.iter_mut() {
      component.init(tui.size().unwrap()).unwrap();
    }

    loop {
      if let Some(e) = tui.next().await {
        match e {
          sazid_tui::Event::Quit => action_tx.send(Action::Quit).unwrap(),
          sazid_tui::Event::Tick => action_tx.send(Action::Tick).unwrap(),
          sazid_tui::Event::Render => action_tx.send(Action::Render).unwrap(),
          sazid_tui::Event::Resize(x, y) => {
            action_tx.send(Action::Resize(x, y)).unwrap()
          },
          sazid_tui::Event::Key(key) => {
            if let Some(keymap) = self.sazid_config.keybindings.get(&self.mode)
            {
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
        let cursor_kind = tui.cursor_kind();
        let cursor_pos =
          if let Ok(cursor) = tui.get_cursor() { Some(cursor) } else { None };

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

            let buf = tui.current_buffer_mut();
            for component in self.components.iter_mut() {
              let r = component.draw(buf);
              if let Err(e) = r {
                action_tx
                  .send(Action::Error(format!("Failed to draw: {:?}", e)))
                  .unwrap();
              }
            }

            tui.draw(cursor_pos, cursor_kind).expect("Failed to draw")
          },
          Action::Render => {
            //trace_dbg!("Action::Render");
            let buf = tui.current_buffer_mut();
            for component in self.components.iter_mut() {
              let r = component.draw(buf);
              if let Err(e) = r {
                action_tx
                  .send(Action::Error(format!("Failed to draw: {:?}", e)))
                  .unwrap();
              }
            }
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
        tui = sazid_tui::Tui::new().unwrap();
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
