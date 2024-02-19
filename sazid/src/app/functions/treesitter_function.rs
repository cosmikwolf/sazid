use crate::app::session_config::SessionConfig;
use std::collections::HashMap;
use std::path::PathBuf;
use tree_sitter::{Parser, Query};

use super::argument_validation::*;
use super::tool_call::ToolCallTrait;
use super::{
  types::{FunctionCall, FunctionParameters, FunctionProperties},
  ToolCallError,
};

use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TreesitterFunction {
  name: String,
  description: String,
  required_properties: Vec<FunctionProperties>,
  optional_properties: Vec<FunctionProperties>,
}

impl ToolCallTrait for TreesitterFunction {
  fn init() -> Self {
    TreesitterFunction {
      name: "treesitter".to_string(),
      description: "parse source code using tree-sitter".to_string(),
      required_properties: vec![FunctionProperties {
        name: "path_globs".to_string(),
        required: true,
        property_type: "string".to_string(),
        description: Some(
          "a comma separated list of glob patterns that represent source files to be parsed".to_string(),
        ),
        enum_values: None,
      }],
      optional_properties: vec![FunctionProperties {
        name: "query".to_string(),
        required: false,
        property_type: "string".to_string(),
        description: Some("tree sitter query to execute upon source file".into()),
        enum_values: None,
      }],
    }
  }

  fn call(
    &self,
    function_args: HashMap<String, serde_json::Value>,
    session_config: SessionConfig,
  ) -> Result<Option<String>, ToolCallError> {
    let paths = validate_and_extract_paths_from_argument(
      &function_args,
      session_config,
      true,
      Some(PathBuf::from("./")),
    )?
    .unwrap();
    let query =
      validate_and_extract_string_argument(&function_args, "query", true)?
        .unwrap();
    treesitter_query(paths, &query)
  }

  fn function_definition(&self) -> FunctionCall {
    let mut properties: HashMap<String, FunctionProperties> = HashMap::new();

    self.required_properties.iter().for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });
    self.optional_properties.iter().for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });

    FunctionCall {
      name: self.name.clone(),
      description: Some(self.description.clone()),
      parameters: Some(FunctionParameters {
        param_type: "object".to_string(),
        required: self
          .required_properties
          .clone()
          .into_iter()
          .map(|p| p.name)
          .collect(),
        properties,
      }),
    }
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
