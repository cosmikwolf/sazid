use futures_util::Future;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;

use crate::action::{ChatToolAction, LsiAction};
use crate::app::lsi::query::LsiQuery;

use super::argument_validation::*;
use super::errors::ToolCallError;
use super::tool_call::{ToolCallParams, ToolCallTrait};
use super::types::*;

#[derive(Serialize, Deserialize)]
pub struct LspGetWorkspaceFiles {
  pub name: String,
  pub description: String,
  pub parameters: FunctionProperty,
}

impl ToolCallTrait for LspGetWorkspaceFiles {
  fn init() -> Self
  where
    Self: Sized,
  {
    LspGetWorkspaceFiles {
      name: "lsp_workspace_files".to_string(),
      description: "list the workspace source files that the language server is aware of"
        .to_string(),
      parameters: FunctionProperty::Parameters {
        properties: HashMap::from([(
          "file_name_regex".to_string(),
          FunctionProperty::String {
            required: false,
            description: Some("filter the results with a matching pattern".to_string()),
          },
        )]),
      },
    }
  }

  fn name(&self) -> &str {
    &self.name
  }

  fn parameters(&self) -> FunctionProperty {
    self.parameters.clone()
  }

  fn description(&self) -> String {
    self.description.clone()
  }

  fn call(
    &self,
    params: ToolCallParams,
  ) -> Pin<Box<dyn Future<Output = Result<Option<String>, ToolCallError>> + Send + 'static>> {
    log::info!("LspGetWorkspaceFiles::call");

    let pattern =
      validate_and_extract_pattern_argument(&params.function_args, "file_name_regex", false)
        .expect("error validating regex");

    let workspace_root =
      params.session_config.workspace.expect("workspace not set").workspace_path.clone();

    let lsi_query = LsiQuery {
      workspace_root,
      session_id: params.session_id,
      tool_call_id: params.tool_call_id,
      file_path_regex: pattern.map(|p| p.to_string()),
      ..Default::default()
    };

    Box::pin(async move {
      params
        .tx
        .send(ChatToolAction::LsiRequest(Box::new(LsiAction::GetWorkspaceFiles(lsi_query))))
        .unwrap();
      Ok(None)
    })
  }
}
