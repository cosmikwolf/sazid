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

use serde_json::Value;
use std::any::Any;
use std::collections::HashMap;
use std::path::PathBuf;
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ArrayProperties {
  #[serde(rename = "items")]
  pub items: Box<PropertyType>,
  #[serde(rename = "minItems")]
  pub min_items: Option<usize>,
  #[serde(rename = "maxItems")]
  pub max_items: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct IntegerProperties {
  #[serde(rename = "minimum")]
  pub minimum: Option<i64>,
  #[serde(rename = "maximum")]
  pub maximum: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum PropertyType {
  #[serde(rename = "boolean")]
  Boolean,
  #[serde(rename = "number")]
  Number,
  #[serde(rename = "string")]
  String,
  #[serde(rename = "string")]
  Pattern,
  #[serde(rename = "null")]
  Null,
  #[serde(rename = "string")]
  PathBuf,
  #[serde(rename = "integer")]
  Integer {
    #[serde(rename = "type")]
    type_: String,
    #[serde(flatten)]
    properties: IntegerProperties,
  },
  #[serde(rename = "array")]
  Array {
    #[serde(rename = "type")]
    type_: String,
    #[serde(flatten)]
    properties: ArrayProperties,
  },
  #[serde(rename = "array")]
  Range {
    #[serde(rename = "type")]
    type_: String,
    #[serde(flatten)]
    properties: ArrayProperties,
  },
}

pub fn validate_arguments(
  arguments: HashMap<String, Value>,
  properties: &[FunctionProperty],
  workspace_root: Option<&PathBuf>,
) -> Result<HashMap<String, Option<Box<dyn Any>>>, String> {
  let mut validated_args: HashMap<String, Option<Box<dyn Any>>> =
    HashMap::new();
  for property in properties {
    let arg_value = arguments.get(&property.name);
    match (arg_value, property.required) {
      (Some(value), _) => match &property.property_type {
        PropertyType::Boolean => {
          if let Some(v) = value.as_bool() {
            validated_args.insert(property.name.clone(), Some(Box::new(v)));
          } else {
            return Err(format!(
              "Invalid type for argument '{}'. Expected: bool, Found: {:?}",
              property.name, value
            ));
          }
        },
        PropertyType::Number => {
          if let Some(v) = value.as_number() {
            validated_args
              .insert(property.name.clone(), Some(Box::new(v.clone())));
          } else {
            return Err(format!(
              "Invalid type for argument '{}'. Expected: number, Found: {:?}",
              property.name, value
            ));
          }
        },
        PropertyType::String => {
          if let Some(v) = value.as_str() {
            validated_args
              .insert(property.name.clone(), Some(Box::new(v.to_string())));
          } else {
            return Err(format!(
              "Invalid type for argument '{}'. Expected: string, Found: {:?}",
              property.name, value
            ));
          }
        },
        PropertyType::Array {
          type_: _,
          properties: ArrayProperties { items, min_items, max_items },
        } => {
          if let Some(arr) = value.as_array() {
            if let Some(min) = min_items {
              if arr.len() < *min {
                return Err(format!("Array length for argument '{}' is below the minimum. Expected: {}, Found: {}", property.name, min, arr.len()));
              }
            }
            if let Some(max) = max_items {
              if arr.len() > *max {
                return Err(format!("Array length for argument '{}' exceeds the maximum. Expected: {}, Found: {}", property.name, max, arr.len()));
              }
            }
            let mut validated_arr = Vec::new();
            for item in arr {
              if is_valid_type(item, items) {
                validated_arr.push(item.clone());
              } else {
                return Err(format!("Invalid type for array item in argument '{}'. Expected: {:?}, Found: {:?}", property.name, items, item));
              }
            }
            validated_args
              .insert(property.name.clone(), Some(Box::new(validated_arr)));
          } else {
            return Err(format!(
              "Invalid type for argument '{}'. Expected: array, Found: {:?}",
              property.name, value
            ));
          }
        },
        PropertyType::Null => {
          if value.is_null() {
            validated_args.insert(property.name.clone(), Some(Box::new(())));
          } else {
            return Err(format!(
              "Invalid type for argument '{}'. Expected: null, Found: {:?}",
              property.name, value
            ));
          }
        },
        PropertyType::Integer {
          type_,
          properties: IntegerProperties { minimum, maximum },
        } => {
          if let Some(v) = value.as_i64() {
            if let Some(min) = minimum {
              if v < *min {
                return Err(format!("Value for argument '{}' is below the minimum. Expected: {}, Found: {}", property.name, min, v));
              }
            }
            if let Some(max) = maximum {
              if v > *max {
                return Err(format!("Value for argument '{}' exceeds the maximum. Expected: {}, Found: {}", property.name, max, v));
              }
            }
            validated_args.insert(property.name.clone(), Some(Box::new(v)));
          } else {
            return Err(format!(
              "Invalid type for argument '{}'. Expected: integer, Found: {:?}",
              property.name, value
            ));
          }
        },
        PropertyType::Range {
          type_,
          properties: ArrayProperties { items, min_items, max_items },
        } => {
          if let Some(arr) = value.as_array() {
            if let Some(min) = min_items {
              if arr.len() < *min {
                return Err(format!("Array length for argument '{}' is below the minimum. Expected: {}, Found: {}", property.name, min, arr.len()));
              }
            }
            if let Some(max) = max_items {
              if arr.len() > *max {
                return Err(format!("Array length for argument '{}' exceeds the maximum. Expected: {}, Found: {}", property.name, max, arr.len()));
              }
            }
            let mut validated_arr = Vec::new();
            for item in arr {
              match items.as_ref() {
                PropertyType::Integer {
                  type_,
                  properties: IntegerProperties { minimum, maximum },
                } => {
                  if let Some(v) = item.as_i64() {
                    if let Some(min) = minimum {
                      if v < *min {
                        return Err(format!("Value for range item in argument '{}' is below the minimum. Expected: {}, Found: {}", property.name, min, v));
                      }
                    }
                    if let Some(max) = maximum {
                      if v > *max {
                        return Err(format!("Value for range item in argument '{}' exceeds the maximum. Expected: {}, Found: {}", property.name, max, v));
                      }
                    }
                    validated_arr.push(v);
                  } else {
                    return Err(format!("Invalid type for range item in argument '{}'. Expected: integer, Found: {:?}", property.name, item));
                  }
                },
                _ => {
                  return Err(format!("Invalid type for range item in argument '{}'. Expected: integer, Found: {:?}", property.name, item));
                },
              }
            }
            validated_args
              .insert(property.name.clone(), Some(Box::new(validated_arr)));
          } else {
            return Err(format!(
              "Invalid type for argument '{}'. Expected: range, Found: {:?}",
              property.name, value
            ));
          }
        },
        PropertyType::Pattern => {
          if let Some(pattern_str) = value.as_str() {
            match regex::Regex::new(pattern_str) {
              Ok(regex) => {
                validated_args
                  .insert(property.name.clone(), Some(Box::new(regex)));
              },
              Err(err) => {
                return Err(format!("Invalid regular expression pattern for argument '{}'. Error: {}", property.name, err));
              },
            }
          } else {
            return Err(format!("Invalid type for argument '{}'. Expected: string (regex pattern), Found: {:?}", property.name, value));
          }
        },
        PropertyType::PathBuf => {
          if workspace_root.is_none() {
            return Err(format!(
              "Workspace root is required to validate path argument '{}'",
              property.name
            ));
          }
          if let Some(path_str) = value.as_str() {
            let path = PathBuf::from(path_str);
            if !path.is_absolute() {
              let absolute_path = workspace_root.unwrap().join(path);
              if absolute_path.exists() {
                validated_args
                  .insert(property.name.clone(), Some(Box::new(absolute_path)));
              } else {
                return Err(format!(
                  "Invalid path for argument '{}'. Path does not exist: {:?}",
                  property.name, absolute_path
                ));
              }
            } else if path.exists() {
              validated_args
                .insert(property.name.clone(), Some(Box::new(path)));
            } else {
              return Err(format!(
                "Invalid path for argument '{}'. Path does not exist: {:?}",
                property.name, path
              ));
            }
          } else {
            return Err(format!("Invalid type for argument '{}'. Expected: string (path), Found: {:?}", property.name, value));
          }
        },
      },
      (None, true) => {
        return Err(format!("Missing required argument: '{}'", property.name));
      },
      (None, false) => {
        validated_args.insert(property.name.clone(), None);
      },
    }
  }
  Ok(validated_args)
}

pub fn get_validated_argument<T: Clone + 'static>(
  validated_arguments: &HashMap<String, Option<Box<dyn Any>>>,
  key: &str,
) -> Option<T> {
  validated_arguments.get(key).and_then(|any| {
    any.as_ref().map(|a| a.downcast_ref::<T>().unwrap().clone())
  })
}
fn is_valid_type(value: &Value, expected_type: &PropertyType) -> bool {
  matches!(
    (value, expected_type),
    (Value::Bool(_), PropertyType::Boolean)
      | (Value::Number(_), PropertyType::Number)
      | (Value::String(_), PropertyType::String)
      | (Value::Array(_), PropertyType::Array { .. })
      | (Value::Null, PropertyType::Null)
      | (Value::Number(_), PropertyType::Integer { .. })
      | (Value::String(_), PropertyType::Pattern)
      | (Value::Array(_), PropertyType::Range { .. })
      | (Value::String(_), PropertyType::PathBuf)
  )
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
