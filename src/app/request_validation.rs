use super::tools::utils::ensure_directory_exists;
use crate::trace_dbg;
use async_openai::types::CreateChatCompletionRequest;
use jsonschema::{Draft, JSONSchema};
use serde_json::{to_string_pretty, Value};
use std::error::Error;
use std::path::Path;
use std::{fmt, fs};

// Custom error type to capture and display the path and value at which validation fails
#[derive(Debug)]
struct ValidationError {
  path: String,
  value: Value,
  message: String,
}

impl fmt::Display for ValidationError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(
      f,
      "{:#?}\nPath: {:#?}\nValue:\n{}",
      self.message,
      self.path,
      to_string_pretty(&self.value).unwrap_or_else(|_| "Invalid value".to_string())
    )
  }
}

impl Error for ValidationError {}

impl ValidationError {
  fn _new(path: Vec<&str>, value: &Value, message: &str) -> ValidationError {
    ValidationError {
      path: path.join(" > "),
      value: value.clone(),
      //value: to_string_pretty(value).unwrap_or_else(|_| "Invalid value".to_string()),
      message: message.to_string(),
    }
  }
}
fn validate_schema(schema: &str, json: &str) -> Result<(), Box<dyn Error>> {
  // Parse the string of data into serde_json::Value.
  let schema_yaml: Value = serde_yaml::from_str(schema)?;
  let json_data: Value = serde_json::from_str(json)?;

  // Build a JSON schema.
  let compiled_schema = JSONSchema::options()
    .with_draft(Draft::Draft7)
    .compile(&schema_yaml)
    .map_err(|e| format!("Compilation error: {}", e))?;

  // Validate the JSON data.
  let validation = compiled_schema.validate(&json_data);

  // Check if validation passed or not.
  match validation {
    Ok(_) => Ok(()),
    Err(errors) => {
      // Collect all validation errors with their JSON paths.
      let error_messages: Vec<String> =
        errors.into_iter().map(|e| format!("Error at path {}: {}", e.instance_path, e)).collect();
      Err(format!("Validation errors: {:?}", error_messages).into())
    },
  }
}

pub fn debug_request_validation(request: &CreateChatCompletionRequest) {
  let request_as_json = serde_json::to_string_pretty(request).unwrap();
  let schema = include_str!("../../assets/openapi.yaml");
  match validate_schema(schema, request_as_json.as_str()) {
    Ok(_) => {
      trace_dbg!("no errors found");
    },
    Err(e) => match e.downcast::<ValidationError>() {
      Ok(validation_error) => {
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let failed_requests_dir = ".data/failed_requests";
        match ensure_directory_exists(failed_requests_dir) {
          Ok(_) => {
            let request_file_path = Path::new(failed_requests_dir).join(timestamp + "_failed.json");
            fs::write(request_file_path.clone(), request_as_json).unwrap();
            let debugstr = format!(
              "request failed. failed request saved to\n{:#?}\nErrors:\n{}",
              request_file_path, validation_error
            );
            trace_dbg!(debugstr);
          },
          Err(e) => {
            trace_dbg!("unable to create failed requests directory {:?}", e);
          },
        }
      },
      Err(e) => {
        format!("request failed. Errors:\n{:#?}", e);
      },
    },
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_valid_json_request() -> Result<(), Box<dyn Error>> {
    let json_request = r#"
        {
            "model": "gpt-3.5-turbo",
            "messages": [
                {
                    "role": "system",
                    "content": "You are a helpful assistant."
                },
                {
                    "role": "user",
                    "content": "Tell me a joke."
                }
            ],
            "functions": [
                {
                    "name": "get_current_weather",
                    "description": "Get the current weather",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "location": {
                                "type": "string",
                                "description": "The city and state, e.g. San Francisco, CA"
                            },
                            "format": {
                                "type": "string",
                                "enum": [
                                    "celsius",
                                    "fahrenheit"
                                ],
                                "description": "The temperature unit to use. Infer this from the users location."
                            }
                        },
                        "required": [
                            "location",
                            "format"
                        ]
                    }
                },
                {
                    "name": "get_n_day_weather_forecast",
                    "description": "Get an N-day weather forecast",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "location": {
                                "type": "string",
                                "description": "The city and state, e.g. San Francisco, CA"
                            },
                            "format": {
                                "type": "string",
                                "enum": [
                                    "celsius",
                                    "fahrenheit"
                                ],
                                "description": "The temperature unit to use. Infer this from the users location."
                            },
                            "num_days": {
                                "type": "integer",
                                "description": "The number of days to forecast"
                            }
                        },
                        "required": [
                            "location",
                            "format",
                            "num_days"
                        ]
                    }
                }
            ]
        }
        "#;

    let schema = include_str!("../../assets/create_chat_completion_request_schema_11_6_23.json");
    validate_json_schema(schema, json_request)?;
    Ok(())
  }

  #[test]
  fn test_invalid_json_request() -> Result<(), Box<dyn Error>> {
    let json_request = r#"
            {
                "model": "",
                "messages": [
                    {"role": "system", "content": ""},
                    {"role": "user", "content": ""}
                ]
            }
        "#;

    let schema = include_str!("../../assets/create_chat_completion_request_schema_11_6_23.json");
    let result = validate_json_schema(schema, json_request);
    if let Err(e) = &result {
      println!("Validation error: {}", e);
    }
    assert!(result.is_err());
    Ok(())
  }
}
