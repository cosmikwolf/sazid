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
  pub properties: Vec<FunctionProperty>,
}

impl ToolCallTrait for LspGetWorkspaceFiles {
  fn init() -> Self
  where
    Self: Sized,
  {
    LspGetWorkspaceFiles {
      name: "lsp_workspace_files".to_string(),
      description:
        "list the workspace source files that the language server is aware of"
          .to_string(),
      properties: vec![FunctionProperty {
        name: "file_name_regex".to_string(),
        required: false,
        property_type: PropertyType::Pattern,
        description: Some(
          "filter the results with a matching pattern".to_string(),
        ),
        enum_values: None,
      }],
    }
  }

  fn name(&self) -> &str {
    &self.name
  }

  fn properties(&self) -> Vec<FunctionProperty> {
    self.properties.clone()
  }

  fn description(&self) -> String {
    self.description.clone()
  }

  fn call(
    &self,
    params: ToolCallParams,
  ) -> Pin<
    Box<
      dyn Future<Output = Result<Option<String>, ToolCallError>>
        + Send
        + 'static,
    >,
  > {
    log::info!("LspGetWorkspaceFiles::call");

    let pattern = validate_and_extract_pattern_argument(
      &params.function_args,
      "file_name_regex",
      false,
    )
    .expect("error validating regex");

    let workspace_params =
      params.session_config.workspace.expect("workspace not set");

    let lsi_query = LsiQuery {
      workspace_root: workspace_params.workspace_path.clone(),
      session_id: params.session_id,
      tool_call_id: params.tool_call_id,
      file_path_regex: pattern.map(|p| p.to_string()),
      ..Default::default()
    };

    Box::pin(async move {
      params
        .tx
        .send(ChatToolAction::LsiRequest(Box::new(
          LsiAction::GetWorkspaceFiles(lsi_query),
        )))
        .unwrap();
      // return none, so the tool completes when it receieves a response from the language server
      Ok(None)
    }) // End example call function code
  }

  // this function creates the FunctionCall struct which is used to pass the function to GPT.
  // This code should not change and should be identical for each function call
  fn function_definition(&self) -> ToolCall {
    let mut properties: HashMap<String, FunctionProperty> = HashMap::new();

    self.properties.iter().filter(|p| p.required).for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });
    self.properties.iter().filter(|p| !p.required).for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });

    ToolCall {
      name: self.name.clone(),
      description: Some(self.description.clone()),
      parameters: Some(FunctionParameters {
        param_type: "object".to_string(),
        required: self
          .properties
          .clone()
          .into_iter()
          .filter(|p| p.required)
          .map(|p| p.name)
          .collect(),
        properties,
      }),
    }
  }
}
