use futures_util::Future;
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use tree_sitter::{Parser, Query};

use super::argument_validation::*;
use super::errors::ToolCallError;
use super::tool_call::{ToolCallParams, ToolCallTrait};
use super::types::*;

use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TreesitterFunction {
  name: String,
  description: String,
  properties: Vec<FunctionProperty>,
}

impl ToolCallTrait for TreesitterFunction {
  fn init() -> Self {
    TreesitterFunction {
      name: "treesitter".to_string(),
      description: "parse source code using tree-sitter".to_string(),
    properties: FunctionProperty::Parameters {
        properties: HashMap::from([
                        ( "path_globs".to_string(),
          FunctionProperty::String {
        required: true,
        description: Some(
          "a comma separated list of glob patterns that represent source files to be parsed".to_string(),
        ),
      }),
      ( "query".to_string(),
      FunctionProperty::String {
        required: false,
        description: Some("tree sitter query to execute upon source file".into()),
      }),
       ])
    },
    }
  }

  fn name(&self) -> &str {
    &self.name
  }

  fn parameters(&self) -> Vec<FunctionProperty> {
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
    Box::pin(async move {
      let paths = validate_and_extract_paths_from_argument(
        &params.function_args,
        params.session_config,
        true,
        Some(PathBuf::from("./")),
      )?
      .unwrap();
      let query = validate_and_extract_string_argument(
        &params.function_args,
        "query",
        true,
      )?
      .unwrap();
      treesitter_query(paths, &query)
    })
  }
}

pub fn treesitter_query(
  paths: Vec<PathBuf>,
  query: &str,
) -> Result<Option<String>, ToolCallError> {
  let mut parser = Parser::new();
  let language = tree_sitter_rust::language();
  parser.set_language(language).expect("Error loading Rust grammar");
  let query = &Query::new(language, query).unwrap();
  let mut results = vec![];
  for path in paths {
    let source_code = std::fs::read_to_string(&path).map_err(|e| {
      ToolCallError::new(
        format!("Error: {}  Path: {}", &e, &path.to_str().unwrap()).as_str(),
      )
    })?;
    let parsed =
      parser.parse(&source_code, None).expect("Error parsing source code");
    let root_node = parsed.root_node();
    let mut query_cursor = tree_sitter::QueryCursor::new();

    let query_matches =
      query_cursor.matches(query, root_node, source_code.as_bytes());
    for match_ in query_matches {
      for capture in match_.captures {
        let capture_text = &source_code
          [capture.node.range().start_byte..capture.node.range().end_byte]
          .to_string();
        results.push(capture_text.clone());
      }
    }
  }

  if results.is_empty() {
    Ok(Some("no matches found".into()))
  } else {
    Ok(Some(results.join("\n")))
  }
}
