use futures_util::Future;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;

use crate::action::{ChatToolAction, LsiAction};
use crate::app::lsi::query::{DiagnosticIncludeFlags, LsiQuery};

use super::errors::ToolCallError;
use super::tool_call::{ToolCallParams, ToolCallTrait};
use super::types::*;

#[derive(Serialize, Deserialize)]
pub struct LspGetDiagnostics {
  pub name: String,
  pub description: String,
  pub parameters: FunctionProperty,
}

impl ToolCallTrait for LspGetDiagnostics {
  fn init() -> Self
  where
    Self: Sized,
  {
    LspGetDiagnostics {
        name: "lsp_diagnostics".to_string(),
        description: "get language server diagnostic information".to_string(),
      parameters: FunctionProperty::Parameters {
        properties: HashMap::from([
             ("errors".to_string(),
               FunctionProperty::Bool{ required: true,
               description: Some("include errors in the diagnostic report".to_string()),
            }),
             ("warnings".to_string(),
               FunctionProperty::Bool{ required: true,
               description: Some("include warnings in the diagnostic report".to_string()),
            }),
             ("information".to_string(),
               FunctionProperty::Bool{ required: true,
               description: Some("include diagnostics classified as information in the diagnostic report".to_string()),
            }),
             ("hints".to_string(),
               FunctionProperty::Bool{ required: true,
               description: Some("include diagnostics classified as hints in the diagnostic report".to_string()),

            }),
             ("no_severity".to_string(),
               FunctionProperty::Bool{ required: true,
               description: Some("include diagnostics with no severity classification in the diagnostic report".to_string()),

            }),
             ("file_path_regex".to_string(),
               FunctionProperty::Pattern{ required: false,
               description: Some("include results where the file path matches".to_string()),

            }),
             ("range".to_string(),
               FunctionProperty::String{ required: false,
               description: Some("filter results by byte range in source file, in the format of start_line,start_char,end_line,end_char. this has no effect if file_path_regex is not specified".to_string()),

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
  ) -> Pin<
    Box<
      dyn Future<Output = Result<Option<String>, ToolCallError>>
        + Send
        + 'static,
    >,
  > {
    let validated_arguments =
      validate_arguments(params.function_args, &self.parameters, None)
        .expect("error validating arguments");

    let file_path_regex =
      get_validated_argument(&validated_arguments, "file_path_regex");

    let include_errors =
      get_validated_argument(&validated_arguments, "include_errors");

    let include_warnings =
      get_validated_argument(&validated_arguments, "include_warnings");

    let include_information =
      get_validated_argument(&validated_arguments, "include_information");

    let include_hints =
      get_validated_argument(&validated_arguments, "include_hints");

    let include_no_severity =
      get_validated_argument(&validated_arguments, "include_no_severity");

    let range = get_validated_argument(&validated_arguments, "range");

    let workspace_root = params
      .session_config
      .workspace
      .expect("workspace not set")
      .workspace_path
      .clone();

    Box::pin(async move {
      let query = LsiQuery {
        workspace_root,
        range,
        file_path_regex,
        tool_call_id: params.tool_call_id,
        session_id: params.session_id,
        diagnostic_severity: Some(DiagnosticIncludeFlags {
          include_errors,
          include_warnings,
          include_information,
          include_hints,
          include_no_severity,
        }),
        ..Default::default()
      };

      params
        .tx
        .send(ChatToolAction::LsiRequest(Box::new(LsiAction::GetDiagnostics(
          query,
        ))))
        .unwrap();
      Ok(None)
    })
  }
}
