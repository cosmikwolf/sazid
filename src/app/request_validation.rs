use serde_json::{from_str, Value};
use std::error::Error;

pub fn validate_json_request(json_request: &str) -> Result<(), Box<dyn Error>> {
  let v: Value = from_str(json_request)?;

  // Validate the 'model' field
  let model = v.get("model").ok_or("Missing 'model' field")?.as_str().ok_or("Invalid 'model' type")?;
  if model.is_empty() {
    return Err("Empty 'model' field".into());
  }

  // Validate the 'messages' field
  let messages = v.get("messages").ok_or("Missing 'messages' field")?.as_array().ok_or("Invalid 'messages' type")?;
  for message in messages {
    let role = message.get("role").ok_or("Missing 'role' field")?.as_str().ok_or("Invalid 'role' type")?;
    let content = message.get("content").ok_or("Missing 'content' field")?.as_str().ok_or("Invalid 'content' type")?;
    if role.is_empty() || content.is_empty() {
      return Err("Empty 'role' or 'content' field in messages".into());
    }
  }

  // Validate the 'functions' field if it exists
  if let Some(functions) = v.get("functions") {
    let functions_array = functions.as_array().ok_or("Invalid 'functions' type")?;
    for function in functions_array {
      let name = function
        .get("name")
        .ok_or("Missing 'name' field in functions")?
        .as_str()
        .ok_or("Invalid 'name' type in functions")?;
      if name.is_empty() {
        return Err("Empty 'name' field in functions".into());
      }

      let description = function
        .get("description")
        .ok_or("Missing 'description' field in functions")?
        .as_str()
        .ok_or("Invalid 'description' type in functions")?;
      if description.is_empty() {
        return Err("Empty 'description' field in functions".into());
      }

      let parameters = function
        .get("parameters")
        .ok_or("Missing 'parameters' field in functions")?
        .as_object()
        .ok_or("Invalid 'parameters' type in functions")?;
      for (param_name, param_details) in parameters {
        let param_type = param_details
          .get("type")
          .ok_or(format!("Missing 'type' for parameter '{}'", param_name))?
          .as_str()
          .ok_or(format!("Invalid 'type' type for parameter '{}'", param_name))?;
        if param_type.is_empty() {
          return Err(format!("Empty 'type' field for parameter '{}'", param_name).into());
        }
      }

      let required = function
        .get("required")
        .ok_or("Missing 'required' field in functions")?
        .as_array()
        .ok_or("Invalid 'required' type in functions")?;
      for req in required {
        let req_str = req.as_str().ok_or("Invalid 'required' field type in functions")?;
        if req_str.is_empty() {
          return Err("Empty 'required' field in functions".into());
        }
      }
    }
  }

  Ok(())
}
#[cfg(test)]
mod tests {
  use std::error::Error;

  use super::validate_json_request;

  #[test]
  fn test_valid_json_request() -> Result<(), Box<dyn Error>> {
    let json_request = r#"
        {
            "model": "gpt-3.5-turbo",
            "messages": [
                {"role": "system", "content": "You are a helpful assistant."},
                {"role": "user", "content": "Tell me a joke."}
            ],
            "functions": [
                {
                    "name": "get_current_weather",
                    "description": "Returns the current weather.",
                    "parameters": {
                        "location": {
                            "type": "string",
                            "description": "The location to get the weather for."
                        },
                        "unit": {
                            "type": "string",
                            "enum": ["celsius", "fahrenheit"]
                        }
                    },
                    "required": ["location"]
                }
            ]
        }
        "#;

    validate_json_request(json_request)?;
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

    let result = validate_json_request(json_request);
    assert!(result.is_err());
    Ok(())
  }
}
