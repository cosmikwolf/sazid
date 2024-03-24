use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FunctionProperties {
  #[serde(skip)]
  pub name: String,
  #[serde(skip)]
  pub required: bool,
  #[serde(rename = "type")]
  pub property_type: String,
  pub description: Option<String>,
  // #[serde(skip_serializing_if = "Option::is_none")]
  // pub properties: Option<Box<FunctionProperties>>,
  #[serde(rename = "enum", default)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub enum_values: Option<Vec<String>>,
}

// #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
// pub enum PropertyType {
//   #[serde(rename = "boolean")]
//   Boolean,
//   #[serde(rename = "number")]
//   Number,
//   #[serde(rename = "string")]
//   String,
//   #[serde(rename = "pattern")]
//   Pattern,
//   #[serde(rename = "array")]
//   Array,
//   #[serde(rename = "object")]
//   Object(FunctionProperties),
//   #[serde(rename = "null")]
//   Null,
// }

#[derive(Serialize, Deserialize, Debug)]
pub struct FunctionParameters {
  #[serde(rename = "type")]
  pub param_type: String,
  pub required: Vec<String>,
  pub properties: std::collections::HashMap<String, FunctionProperties>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ToolCall {
  pub name: String,
  pub description: Option<String>,
  pub parameters: Option<FunctionParameters>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Commands {
  pub commands: Vec<ToolCall>,
}
