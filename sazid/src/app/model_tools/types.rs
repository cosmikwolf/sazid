use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

fn serialize_parameters<S>(
  properties: &std::collections::HashMap<String, FunctionProperty>,
  serializer: S,
) -> Result<S::Ok, S::Error>
where
  S: Serializer,
{
  use serde::ser::SerializeStruct;
  let mut state = serializer.serialize_struct("Properties", 3)?;
  state.serialize_field("type", "object")?;
  state.serialize_field("properties", properties)?;
  let required: Vec<String> = properties
    .iter()
    .filter_map(|(key, value)| match value {
      FunctionProperty::Bool { required, .. }
      | FunctionProperty::Number { required, .. }
      | FunctionProperty::String { required, .. }
      | FunctionProperty::Pattern { required, .. }
      | FunctionProperty::Null { required, .. }
      | FunctionProperty::PathBuf { required, .. }
      | FunctionProperty::Array { required, .. }
      | FunctionProperty::Integer { required, .. } => {
        if *required {
          Some(key.clone())
        } else {
          None
        }
      },
      FunctionProperty::Parameters { .. } => None,
    })
    .collect();
  state.serialize_field("required", &required)?;
  state.end()
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum FunctionProperty {
  #[serde(rename = "object")]
  Parameters {
    #[serde(flatten, serialize_with = "serialize_parameters")]
    properties: std::collections::HashMap<String, FunctionProperty>,
  },
  #[serde(rename = "boolean")]
  Bool {
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip)]
    required: bool,
  },
  #[serde(rename = "number")]
  Number {
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip)]
    required: bool,
  },
  #[serde(rename = "string")]
  String {
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip)]
    required: bool,
  },
  #[serde(rename = "string")]
  Pattern {
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip)]
    required: bool,
  },
  #[serde(rename = "null")]
  Null {
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip)]
    required: bool,
  },
  #[serde(rename = "string")]
  PathBuf {
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip)]
    required: bool,
  },
  #[serde(rename = "array")]
  Array {
    #[serde(rename = "items")]
    items: Box<FunctionProperty>,
    #[serde(rename = "minItems")]
    min_items: Option<usize>,
    #[serde(rename = "maxItems")]
    max_items: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip)]
    required: bool,
  },
  #[serde(rename = "number")]
  Integer {
    #[serde(rename = "minimum")]
    minimum: Option<i64>,
    #[serde(rename = "maximum")]
    maximum: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip)]
    required: bool,
  },
}

pub fn validate_arguments(
  arguments: HashMap<String, Value>,
  parameters: &FunctionProperty,
  workspace_root: Option<&PathBuf>,
) -> Result<HashMap<String, Value>, String> {
  let properties = if let FunctionProperty::Parameters { properties } = parameters {
    properties
  } else {
    return Err("parameters must be FunctionProperty::Parameters".to_string());
  };

  let mut validated_args: HashMap<String, Value> = HashMap::new();

  for (name, property) in properties {
    let arg_value = arguments.get(name);

    match (arg_value, property) {
      (Some(value), FunctionProperty::Bool { required: _, .. }) => {
        if let Some(v) = value.as_bool() {
          validated_args.insert(name.clone(), Value::Bool(v));
        } else {
          return Err(format!(
            "Invalid type for argument '{}'. Expected: bool, Found: {:?}",
            name, value
          ));
        }
      },
      (Some(value), FunctionProperty::Number { required: _, .. }) => {
        if let Some(v) = value.as_f64() {
          validated_args.insert(name.clone(), serde_json::Number::from_f64(v).into());
        } else {
          return Err(format!(
            "Invalid type for argument '{}'. Expected: number, Found: {:?}",
            name, value
          ));
        }
      },
      (Some(value), FunctionProperty::String { required: _, .. }) => {
        if let Some(v) = value.as_str() {
          validated_args.insert(name.clone(), Value::String(v.to_string()));
        } else {
          return Err(format!(
            "Invalid type for argument '{}'. Expected: string, Found: {:?}",
            name, value
          ));
        }
      },
      (Some(value), FunctionProperty::Array { items, min_items, max_items, required: _, .. }) => {
        if let Some(arr) = value.as_array() {
          if let Some(min) = min_items {
            if arr.len() < *min {
              return Err(format!(
                "Array length for argument '{}' is below the minimum. Expected: {}, Found: {}",
                name,
                min,
                arr.len()
              ));
            }
          }
          if let Some(max) = max_items {
            if arr.len() > *max {
              return Err(format!(
                "Array length for argument '{}' exceeds the maximum. Expected: {}, Found: {}",
                name,
                max,
                arr.len()
              ));
            }
          }
          let mut validated_arr = Vec::new();
          for item in arr {
            if is_valid_type(item, items) {
              validated_arr.push(item.clone());
            } else {
              return Err(format!(
                "Invalid type for array item in argument '{}'. Expected: {:?}, Found: {:?}",
                name, items, item
              ));
            }
          }
          validated_args.insert(name.clone(), Value::Array(validated_arr));
        } else {
          return Err(format!(
            "Invalid type for argument '{}'. Expected: array, Found: {:?}",
            name, value
          ));
        }
      },
      (Some(value), FunctionProperty::Null { required: _, .. }) => {
        if value.is_null() {
          validated_args.insert(name.clone(), Value::Null);
        } else {
          return Err(format!(
            "Invalid type for argument '{}'. Expected: null, Found: {:?}",
            name, value
          ));
        }
      },
      (Some(value), FunctionProperty::Integer { minimum, maximum, required: _, .. }) => {
        if let Some(v) = value.as_i64() {
          if let Some(min) = minimum {
            if v < *min {
              return Err(format!(
                "Value for argument '{}' is below the minimum. Expected: {}, Found: {}",
                name, min, v
              ));
            }
          }
          if let Some(max) = maximum {
            if v > *max {
              return Err(format!(
                "Value for argument '{}' exceeds the maximum. Expected: {}, Found: {}",
                name, max, v
              ));
            }
          }
          validated_args.insert(name.clone(), Value::Number(v.into()));
        } else {
          return Err(format!(
            "Invalid type for argument '{}'. Expected: integer, Found: {:?}",
            name, value
          ));
        }
      },
      (Some(value), FunctionProperty::Pattern { required: _, .. }) => {
        if let Some(pattern_str) = value.as_str() {
          match regex::Regex::new(pattern_str) {
            Ok(_regex) => {
              validated_args.insert(name.clone(), Value::String(pattern_str.to_string()));
            },
            Err(err) => {
              return Err(format!(
                "Invalid regular expression pattern for argument '{}'. Error: {}",
                name, err
              ));
            },
          }
        } else {
          return Err(format!(
            "Invalid type for argument '{}'. Expected: string (regex pattern), Found: {:?}",
            name, value
          ));
        }
      },
      (Some(value), FunctionProperty::PathBuf { required: _, .. }) => {
        if workspace_root.is_none() {
          return Err(format!("Workspace root is required to validate path argument '{}'", name));
        }

        if let Some(path_str) = value.as_str() {
          let path = PathBuf::from(path_str);

          // return an error if path is not within workspace
          match workspace_root {
            Some(workspace_dir) => {
              if !path.starts_with(workspace_dir) {
                return Err("cannot read files outside of the current working directory".into());
              }
            },
            None => return Err("cannot create files without a workspace set".into()),
          }

          if !path.is_absolute() {
            let absolute_path = workspace_root.unwrap().join(path);
            if absolute_path.exists() {
              validated_args
                .insert(name.clone(), Value::String(absolute_path.to_string_lossy().into_owned()));
            } else {
              return Err(format!(
                "Invalid path for argument '{}'. Path does not exist: {:?}",
                name, absolute_path
              ));
            }
          } else if path.exists() {
            validated_args.insert(name.clone(), Value::String(path.to_string_lossy().into_owned()));
          } else {
            return Err(format!(
              "Invalid path for argument '{}'. Path does not exist: {:?}",
              name, path
            ));
          }
        } else {
          return Err(format!(
            "Invalid type for argument '{}'. Expected: string (path), Found: {:?}",
            name, value
          ));
        }
      },
      (None, FunctionProperty::Bool { required: true, .. })
      | (None, FunctionProperty::Number { required: true, .. })
      | (None, FunctionProperty::String { required: true, .. })
      | (None, FunctionProperty::Array { required: true, .. })
      | (None, FunctionProperty::Null { required: true, .. })
      | (None, FunctionProperty::Integer { required: true, .. })
      | (None, FunctionProperty::Pattern { required: true, .. })
      | (None, FunctionProperty::PathBuf { required: true, .. }) => {
        return Err(format!("Missing required argument: '{}'", name));
      },
      (None, FunctionProperty::Bool { required: false, .. })
      | (None, FunctionProperty::Number { required: false, .. })
      | (None, FunctionProperty::String { required: false, .. })
      | (None, FunctionProperty::Array { required: false, .. })
      | (None, FunctionProperty::Null { required: false, .. })
      | (None, FunctionProperty::Integer { required: false, .. })
      | (None, FunctionProperty::Pattern { required: false, .. })
      | (None, FunctionProperty::PathBuf { required: false, .. }) => {
        // Skip optional arguments that are not provided
      },
      (_, FunctionProperty::Parameters { .. }) => {
        // Skip validation for nested properties
      },
    }
  }

  Ok(validated_args)
}

pub fn get_validated_argument<T: serde::de::DeserializeOwned>(
  validated_arguments: &HashMap<String, Value>,
  key: &str,
) -> Option<T> {
  validated_arguments.get(key).and_then(|value| serde_json::from_value(value.clone()).ok())
}

fn is_valid_type(value: &Value, expected_type: &FunctionProperty) -> bool {
  matches!(
    (value, expected_type),
    (Value::Bool(_), FunctionProperty::Bool { .. })
      | (Value::Number(_), FunctionProperty::Number { .. })
      | (Value::String(_), FunctionProperty::String { .. })
      | (Value::Array(_), FunctionProperty::Array { .. })
      | (Value::Null, FunctionProperty::Null { .. })
      | (Value::Number(_), FunctionProperty::Integer { .. })
      | (Value::String(_), FunctionProperty::Pattern { .. })
      | (Value::String(_), FunctionProperty::PathBuf { .. })
  )
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ToolCall {
  pub name: String,
  pub description: Option<String>,
  pub parameters: Option<FunctionProperty>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Commands {
  pub commands: Vec<ToolCall>,
}
