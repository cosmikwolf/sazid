use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FunctionProperty {
  #[serde(skip)]
  pub name: String,
  #[serde(skip)]
  pub required: bool,
  #[serde(rename = "type")]
  pub property_type: PropertyType,
  pub description: Option<String>,
  // #[serde(skip_serializing_if = "Option::is_none")]
  // pub properties: Option<Box<FunctionProperties>>,
  #[serde(rename = "enum", default)]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub enum_values: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum PropertyType {
  #[serde(rename = "boolean")]
  Boolean,
  #[serde(rename = "number")]
  Number,
  #[serde(rename = "string")]
  String,
  #[serde(rename = "pattern")]
  Pattern,
  #[serde(rename = "array")]
  Array,
  #[serde(rename = "array")]
  ParametersArray(Vec<FunctionParameters>),
  #[serde(rename = "object")]
  Object(Box<FunctionParameters>),
  #[serde(rename = "null")]
  Null,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct FunctionParameters {
  #[serde(rename = "type")]
  pub param_type: String,
  pub required: Vec<String>,
  pub properties: std::collections::HashMap<String, FunctionProperty>,
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
