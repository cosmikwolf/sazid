use futures_util::Future;
use lsp_types::{Range, SymbolKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;

use crate::action::{ChatToolAction, LsiAction};
use crate::app::lsi::get_file_range_contents;
use crate::app::lsi::query::LsiQuery;

use super::errors::ToolCallError;
use super::tool_call::{ToolCallParams, ToolCallTrait};
use super::types::*;

#[derive(Serialize, Deserialize)]
pub struct ReadFileText {
  pub name: String,
  pub description: String,
  pub parameters: FunctionProperty,
}

impl ToolCallTrait for ReadFileText {
  fn init() -> Self
  where
    Self: Sized,
  {
    ReadFileText {
            name: "read_file".to_string(),
            description: "read text from a file".to_string(),
            parameters: FunctionProperty::Parameters {
            properties: HashMap::from([
                    ("file_path".to_string(),
              FunctionProperty::PathBuf{
                    required: true,
                    description: Some("path of file to read text from".to_string()),
                }),
                    ("byte_range".to_string(),
              FunctionProperty::String {
                    required: false,
                    description: Some("range of bytes to read from file, in the format of start_line,start_char,end_line,end_char. omit to read entire file".to_string()),
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

    let file_path = get_validated_argument::<PathBuf>(&validated_arguments, "file_path")
      .expect("file_path is required");
    let range = get_validated_argument::<Range>(&validated_arguments, "range");

    Box::pin(async move {
      Ok(Some(get_file_range_contents(&file_path, range).expect("unable to read file contents")))
    }) // End example call function code
  }
}
