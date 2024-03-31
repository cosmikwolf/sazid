use std::{collections::HashMap, fs, io::{self, ErrorKind}, process::Command as ProcessCommand};
use serde::{Deserialize, Serialize};
use crate::app::functions::types::{Command, CommandParameters, CommandProperty};
use crate::app::functions::{ModelFunction, ModelFunctionError};
use crate::app::session_config::SessionConfig;

const COMMANDS_CONFIG: &str = "./.session_data/commands.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericCLICommandConfig {
    name: String,
    description: String,
    required_arguments: Vec<String>,
    optional_arguments: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct GenericCLICommand {
    pub configs: Vec<GenericCLICommandConfig>,
}

impl ModelFunction for GenericCLICommand {
    fn init() -> Self {
        let configs: Vec<GenericCLICommandConfig> =
            fs::read_to_string(COMMANDS_CONFIG)
                .map_err(|e| io::Error::new(ErrorKind::NotFound, e))
                .and_then(|contents| toml::from_str(&contents).map_err(|e| io::Error::new(ErrorKind::InvalidData, e)))
                .unwrap_or_default();
        GenericCLICommand { configs }
    }

    fn call(
        &self,
        _function_args: HashMap<String, serde_json::Value>,
        _session_config: SessionConfig,
    ) -> Result<Option<String>, ModelFunctionError> {
        Err(ModelFunctionError::new("call is not used for GenericCLICommand"))
    }

    fn command_definition(&self) -> Command {
        self.configs.iter().map(|config| {
            let mut properties: HashMap<String, CommandProperty> = HashMap::new();
            config.required_arguments.iter().for_each(|arg| {
                properties.insert(
                    arg.clone(),
                    CommandProperty {
                        name: arg.clone(),
                        required: true,
                        property_type: "string".to_string(),
                        description: Some(format!("Required argument for command {}", config.name)),
                        enum_values: None,
                    },
                );
            });

            if let Some(ref optional_arguments) = config.optional_arguments {
                optional_arguments.keys().for_each(|arg| {
                    properties.insert(
                        arg.clone(),
                        CommandProperty {
                            name: arg.clone(),
                            required: false,
                            property_type: "string".to_string(),
                            description: Some(format!("Optional argument for command {}", config.name)),
                            enum_values: None,
                        },
                    );
                });
            }

            (config.name.clone(), Command {
                name: config.name.clone(),
                description: Some(config.description.clone()),
                parameters: Some(CommandParameters {
                    param_type: "object".to_string(),
                    required: config.required_arguments.clone(),
                    properties,
                }),
            })
        }).collect::<HashMap<_, _>>()
    }
}