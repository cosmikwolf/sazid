use std::{
  collections::HashMap,
  fs::{self, File},
  io::Write,
  path::PathBuf,
};

use crate::app::session_config::SessionConfig;
use serde_derive::{Deserialize, Serialize};

use super::{
  types::{Command, CommandParameters, CommandProperty},
  FunctionCall, FunctionCallError,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PatchFilesFunction {
  name: String,
  description: String,
  required_properties: Vec<CommandProperty>,
  optional_properties: Vec<CommandProperty>,
}

impl FunctionCall for PatchFilesFunction {
  fn init() -> Self {
    PatchFilesFunction {
      name: "patch_files".to_string(),
      description: "optionally create and compulsorily apply a unified diff format .patch file".to_string(),
      required_properties: vec![CommandProperty {
        name: "patch_name".to_string(),
        required: true,
        property_type: "string".to_string(),
        description: Some("name of .patch file, including extension, in the .session_data/patches folder".to_string()),
        enum_values: None,
      }],
      optional_properties: vec![CommandProperty {
        name: "patch_content".to_string(),
        required: true,
        property_type: "string".to_string(),
        description: Some(".patch file content in unified diff format".to_string()),
        enum_values: None,
      }],
    }
  }

  fn call(
    &self,
    function_args: HashMap<String, serde_json::Value>,
    session_config: SessionConfig,
  ) -> Result<Option<String>, FunctionCallError> {
    let patches_dir = session_config.session_dir.join("patches");
    // ensure the patches directory and session_data dir exist
    if !patches_dir.exists() {
      fs::create_dir_all(patches_dir)?;
    }

    match function_args.get("patch_name").and_then(|s| s.as_str()) {
      Some(patch_name) => {
        let patch_path = session_config.session_dir.join("patches").join(patch_name);
        let create_patch_file_results =
          if let Some(patch_content) = function_args.get("patch_content").and_then(|s| s.as_str()) {
            create_patch_file(patch_path.clone(), patch_content)
          } else {
            Ok("".to_string())
          };
        let create_patch_output = match create_patch_file_results {
          Ok(output) => output,
          Err(e) => format!("create patch file error: {}", e).to_string(),
        };
        let apply_patch_results = apply_patch_file(patch_path)?;
        Ok(Some(format!("{}{}", create_patch_output, apply_patch_results)))
      },
      None => Ok(Some("patch_name argument is required".to_string())),
    }
  }

  fn command_definition(&self) -> Command {
    let mut properties: HashMap<String, CommandProperty> = HashMap::new();

    self.required_properties.iter().for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });
    self.optional_properties.iter().for_each(|p| {
      properties.insert(p.name.clone(), p.clone());
    });

    Command {
      name: self.name.clone(),
      description: Some(self.description.clone()),
      parameters: Some(CommandParameters {
        param_type: "object".to_string(),
        required: self.required_properties.clone().into_iter().map(|p| p.name).collect(),
        properties,
      }),
    }
  }
}

pub fn apply_patch_file(patch_path: PathBuf) -> Result<String, FunctionCallError> {
  let mut command = std::process::Command::new("patch");
  command.arg("-p1");
  command.arg("-i");
  command.arg(patch_path);
  let output = command.output()?;
  Ok(format!("output: {}", String::from_utf8_lossy(&output.stdout)))
}

pub fn create_patch_file(patch_path: PathBuf, patch_content: &str) -> Result<String, FunctionCallError> {
  match File::create(patch_path.clone()) {
    Ok(mut file) => match file.write_all(patch_content.as_bytes()) {
      Ok(_) => Ok("patch file created\n".to_string()),
      Err(e) => Ok(format!("error writing file: {}\n", e)),
    },
    Err(e) => Ok(format!("error creating file at {}, error: {}\n", patch_path.display(), e)),
  }
}
