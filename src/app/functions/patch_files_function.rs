use patch::{Line, Patch};
use std::{
  collections::HashMap,
  fs::{self, File},
  io::Write,
  path::PathBuf,
};

use crate::app::session_config::SessionConfig;
use serde_derive::{Deserialize, Serialize};

use super::{
  function_call::ModelFunction,
  types::{Command, CommandParameters, CommandProperty},
  ModelFunctionError,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PatchFilesFunction {
  name: String,
  description: String,
  required_properties: Vec<CommandProperty>,
  optional_properties: Vec<CommandProperty>,
}

impl ModelFunction for PatchFilesFunction {
  fn init() -> Self {
    PatchFilesFunction {
      name: "patch_files".to_string(),
      description: "optionally create and compulsorily apply a unified diff format .patch file".to_string(),
      required_properties: vec![
        CommandProperty {
          name: "file_to_patch".to_string(),
          required: true,
          property_type: "string".to_string(),
          description: Some("the path to the file which the patch will be applied to".to_string()),
          enum_values: None,
        },
        CommandProperty {
          name: "patch_name".to_string(),
          required: true,
          property_type: "string".to_string(),
          description: Some(
            "name of .patch file, including extension. patch must be in the .session_data/patches folder".to_string(),
          ),
          enum_values: None,
        },
      ],
      optional_properties: vec![CommandProperty {
        name: "patch_content".to_string(),
        required: false,
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
  ) -> Result<Option<String>, ModelFunctionError> {
    let patches_dir = session_config.session_dir.join("patches");
    // ensure the patches directory and session_data dir exist
    if !patches_dir.exists() {
      fs::create_dir_all(patches_dir)?;
    }
    let file_path = function_args
      .get("file_to_patch")
      .and_then(|s| s.as_str())
      .map(PathBuf::from)
      .ok_or_else(|| ModelFunctionError::new("file_to_patch argument is required"))?;

    match function_args.get("patch_name").and_then(|s| s.as_str()) {
      Some(patch_name) => {
        let patch_path = session_config.session_dir.join("patches").join(patch_name);
        let create_patch_file_results =
          if let Some(patch_content) = function_args.get("patch_content").and_then(|s| s.as_str()) {
            create_patch_file(patch_path.clone(), patch_content)
          } else {
            Ok("new patch content not present".to_string())
          };
        let create_patch_output = match create_patch_file_results {
          Ok(output) => output,
          Err(e) => format!("create patch file error: {}", e).to_string(),
        };
        let apply_patch_results = apply_patch_file(file_path, patch_path)?;
        Ok(Some(format!("patch_content output: {}\tpatch_file output: {}", create_patch_output, apply_patch_results)))
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

fn apply(diff: Patch, old: &str) -> String {
  let old_lines = old.lines().collect::<Vec<&str>>();
  let mut out: Vec<&str> = vec![];
  let mut old_line = 0;
  for hunk in diff.hunks {
    while old_line < hunk.old_range.start - 1 {
      out.push(old_lines[old_line as usize]);
      old_line += 1;
    }
    old_line += hunk.old_range.count;
    for line in hunk.lines {
      match line {
        Line::Add(s) | Line::Context(s) => out.push(s),
        Line::Remove(_) => {},
      }
    }
  }
  out.join("\n")
}

pub fn apply_patch_file(file_path: PathBuf, patch_path: PathBuf) -> Result<String, ModelFunctionError> {
  let original_content = match fs::read_to_string(&file_path) {
    Ok(content) => content,
    Err(e) => return Err(ModelFunctionError::new(&format!("error reading original file: {}", e))),
  };

  let patch_content = match fs::read_to_string(patch_path) {
    Ok(content) => content,
    Err(e) => return Err(ModelFunctionError::new(&format!("error reading patch file: {}", e))),
  };

  let patch = match Patch::from_single(&patch_content) {
    Ok(patch) => patch,
    Err(e) => return Err(ModelFunctionError::new(&format!("error parsing patch content: {}", e))),
  };

  let patched_content = apply(patch, &original_content);

  match fs::write(&file_path, patched_content) {
    Ok(()) => Ok("Patch applied successfully".to_string()),
    Err(e) => Err(ModelFunctionError::new(&format!("error writing patched file: {}", e))),
  }
}

pub fn create_patch_file(patch_path: PathBuf, patch_content: &str) -> Result<String, ModelFunctionError> {
  match File::create(patch_path.clone()) {
    Ok(mut file) => match file.write_all(patch_content.as_bytes()) {
      Ok(_) => Ok("patch file created\n".to_string()),
      Err(e) => Err(ModelFunctionError::new(&format!("error writing file: {}\n", e))),
    },
    Err(e) => Err(ModelFunctionError::new(&format!("error creating file at {}, error: {}\n", patch_path.display(), e))),
  }
}
