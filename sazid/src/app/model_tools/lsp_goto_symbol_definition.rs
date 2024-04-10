use futures_util::Future;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;

use crate::action::{ChatToolAction, LsiAction};
use crate::app::lsi::query::LsiQuery;

use super::errors::ToolCallError;
use super::tool_call::{ToolCallParams, ToolCallTrait};
use super::types::*;

#[derive(Serialize, Deserialize)]
pub struct LspGotoSymbolDefinition {
  pub name: String,
  pub description: String,
  pub parameters: FunctionProperty,
}

impl ToolCallTrait for LspGotoSymbolDefinition {
  fn init() -> Self
  where
    Self: Sized,
  {
    LspGotoSymbolDefinition {
      name: "lsp_goto_symbol_definition".to_string(),
      description: "get the symbol information for where a symbol is defined".to_string(),
      parameters: FunctionProperty::Parameters {
        properties: HashMap::from([(
          "symbol_id".to_string(),
          FunctionProperty::Array {
            required: true,
            description: Some(
              "the 32 byte symbol_id for which to find the declaration".to_string(),
            ),
            items: Box::new(FunctionProperty::Integer {
              description: None,
              required: true,
              minimum: Some(0),
              maximum: Some(255),
            }),
            min_items: Some(32),
            max_items: Some(32),
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
    let validated_arguments = validate_arguments(params.function_args, &self.parameters, None)
      .expect("error validating arguments");

    let symbol_id = get_validated_argument(&validated_arguments, "symbol_id");

    let workspace_root =
      params.session_config.workspace.expect("workspace not set").workspace_path.clone();

    Box::pin(async move {
      let query = LsiQuery {
        symbol_id,
        workspace_root,

        tool_call_id: params.tool_call_id,
        session_id: params.session_id,
        ..Default::default()
      };

      params
        .tx
        .send(ChatToolAction::LsiRequest(Box::new(LsiAction::GoToSymbolDefinition(query))))
        .unwrap();
      Ok(None)
    })
  }
}
