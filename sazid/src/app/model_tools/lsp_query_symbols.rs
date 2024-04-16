use futures_util::Future;
use lsp_types::{Range, SymbolKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;

use crate::action::{ChatToolAction, LsiAction};
use crate::app::lsi::query::LsiQuery;

use super::errors::ToolCallError;
use super::tool_call::{ToolCallParams, ToolCallTrait};
use super::types::*;

#[derive(Serialize, Deserialize)]
pub struct LspQuerySymbol {
  pub name: String,
  pub description: String,
  pub parameters: FunctionProperty,
}

impl ToolCallTrait for LspQuerySymbol {
  fn init() -> Self
  where
    Self: Sized,
  {
    LspQuerySymbol {
            name: "lsp_query".to_string(),
            description: "query symbols in project source code using a language server. each property will filter the results. Omit a property to query for unfiltered results".to_string(),
              parameters: FunctionProperty::Parameters {
          properties: HashMap::from([
                    ("name_regex".to_string(),
                  FunctionProperty::Pattern {
                    required: false,
                    description: Some("include results where the symbol name matches".to_string()),
                }),
                    ("kind".to_string(),
              FunctionProperty::String {
                    required: false,
                description: Some("filter results by kind: MODULE NAMESPACE PACKAGE CLASS METHOD PROPERTY FIELD CONSTRUCTOR ENUM INTERFACE FUNCTION VARIABLE CONSTANT STRING NUMBER BOOLEAN ARRAY OBJECT KEY NULL ENUM_MEMBER STRUCT EVENT OPERATOR TYPE_PARAMETER".to_string()),
                }),
                    ("range".to_string(),
              FunctionProperty::String {
                    required: false,
                description: Some("filter results by byte range in source file, in the format of start_line,start_char,end_line,end_char".to_string()),
                }),
                    ("file_path_regex".to_string(),
              FunctionProperty::Pattern {
                    required: false,
                description: Some("include results where the file path matches".to_string()),
                }),
            ]),
      }
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

    let workspace_root = params
      .session_config
      .workspace
      .as_ref()
      .expect("workspace not set")
      .workspace_path
      .to_path_buf();

    let name_regex = get_validated_argument::<String>(&validated_arguments, "name_regex");
    let kind = get_validated_argument::<String>(&validated_arguments, "kind");
    let range = get_validated_argument::<Range>(&validated_arguments, "range");

    let file_path_regex = get_validated_argument::<String>(&validated_arguments, "file_path_regex");

    Box::pin(async move {
      params.session_config.workspace.expect("workspace must be initialized before query");

      let kind: Option<SymbolKind> = kind.and_then(|kind| {
        let kind = change_case::pascal_case(&kind);
        SymbolKind::try_from(kind.as_str()).ok()
      });

      let query = LsiQuery {
        name_regex,
        kind,
        range,
        workspace_root,
        tool_call_id: params.tool_call_id,
        session_id: params.session_id,
        file_path_regex,
        diagnostic_severity: None,
        ..Default::default()
      };

      params
        .tx
        .send(ChatToolAction::LsiRequest(Box::new(LsiAction::QueryWorkspaceSymbols(query))))
        .unwrap();
      Ok(None)
    }) // End example call function code
  }
}
