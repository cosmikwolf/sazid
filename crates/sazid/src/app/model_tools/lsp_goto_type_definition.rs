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
pub struct LspGotoTypeDefinition {
  pub name: String,
  pub description: String,
  pub properties: Vec<FunctionProperty>,
}

impl ToolCallTrait for LspGotoTypeDefinition {
  fn init() -> Self
  where
    Self: Sized,
  {
    LspGotoTypeDefinition {
      name: "lsp_goto_type_definition".to_string(),
      description:
        "get the symbol information for where a symbol type is defined"
          .to_string(),
      properties: vec![FunctionProperty {
        name: "symbol_id".to_string(),
        required: true,
        property_type: PropertyType::Array {
          type_: "integer".to_string(),
          properties: ArrayProperties {
            items: Box::new(PropertyType::Integer {
              type_: "integer".to_string(),
              properties: IntegerProperties {
                minimum: Some(0),
                maximum: Some(255),
              },
            }),
            min_items: Some(32),
            max_items: Some(32),
          },
        },
        description: Some(
          "the 32 byte symbol_id for which to find the declaration".to_string(),
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
    let validated_arguments =
      validate_arguments(params.function_args, &self.properties, None)
        .expect("error validating arguments");

    let symbol_id: Option<Vec<u8>> =
      get_validated_argument(&validated_arguments, "symbol_id");

    params.session_config.workspace.expect("workspace not set");

    Box::pin(async move {
      // Begin Example Call Code
      // This command is an abstraction for a CLI command, so it calls std::process::command, any new function should have whatever implementation is necessary to execute the function, and should return a Result<Option<String>, ToolCallError>
      // If the code is too complex, it should be broken out into another function.

      let query = LsiQuery {
        symbol_id,

        tool_call_id: params.tool_call_id,
        session_id: params.session_id,
        ..Default::default()
      };

      params
        .tx
        .send(ChatToolAction::LsiRequest(Box::new(
          LsiAction::GoToTypeDefinition(query),
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
